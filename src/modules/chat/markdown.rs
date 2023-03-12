use anyhow::Ok;
use pulldown_cmark::{
    CodeBlockKind, CowStr, Event as CmarkEvent, Options as CmarkOptions, Parser as CmarkParser,
    Tag as CmarkTag,
};
use teloxide::types::{MessageEntity, MessageEntityKind};

#[derive(Debug, Default)]
pub struct ParsedString {
    pub content: String,
    pub entities: Vec<MessageEntity>,
}

enum Event<'a> {
    Start(Tag<'a>),
    End(Tag<'a>),
    Text(CowStr<'a>),
    Code(CowStr<'a>),
    Break,
}

#[derive(Debug)]
enum Tag<'a> {
    Paragraph,
    Heading(u32),
    CodeBlock(Option<CowStr<'a>>),
    List(Option<u64>),
    Item,
    Italic,
    Bold,
    Strikethrough,
    Link(CowStr<'a>),
    Image(CowStr<'a>),
}

impl<'a> TryFrom<CmarkTag<'a>> for Tag<'a> {
    type Error = anyhow::Error;

    fn try_from(value: CmarkTag<'a>) -> Result<Self, Self::Error> {
        let mapped = match value {
            CmarkTag::Paragraph | CmarkTag::BlockQuote => Tag::Paragraph,
            CmarkTag::Heading(level, _, _) => Tag::Heading(level as _),
            CmarkTag::CodeBlock(code_block_kind) => match code_block_kind {
                CodeBlockKind::Indented => Tag::CodeBlock(None),
                CodeBlockKind::Fenced(lang) => Tag::CodeBlock(Some(lang)),
            },
            CmarkTag::List(first) => Tag::List(first),
            CmarkTag::Item => Tag::Item,
            CmarkTag::Emphasis => Tag::Italic,
            CmarkTag::Strong => Tag::Bold,
            CmarkTag::Strikethrough => Tag::Strikethrough,
            CmarkTag::Link(_, url, _) => Tag::Link(url),
            CmarkTag::Image(_, url, _) => Tag::Image(url),
            _ => return Err(anyhow!("Unexpected tag: {:?}", value)),
        };
        Ok(mapped)
    }
}

impl<'a> TryFrom<CmarkEvent<'a>> for Event<'a> {
    type Error = anyhow::Error;

    fn try_from(value: CmarkEvent<'a>) -> Result<Self, Self::Error> {
        let mapped = match value {
            CmarkEvent::Start(tag) => Event::Start(tag.try_into()?),
            CmarkEvent::End(tag) => Event::End(tag.try_into()?),
            CmarkEvent::Text(text) => Event::Text(text),
            CmarkEvent::Html(text) | CmarkEvent::Code(text) => Event::Code(text),
            CmarkEvent::SoftBreak | CmarkEvent::HardBreak => Event::Break,
            CmarkEvent::Rule => Event::Text(CowStr::Borrowed("---")),
            _ => {
                return Err(anyhow!("Unexpected event: {:?}", value));
            }
        };
        Ok(mapped)
    }
}

#[derive(Debug)]
enum EntityKind {
    TelegramEntityKind(MessageEntityKind),
    List(Option<u64>),
}

impl<'a> TryFrom<Tag<'a>> for EntityKind {
    type Error = anyhow::Error;

    fn try_from(value: Tag<'a>) -> Result<Self, Self::Error> {
        let mapped = match value {
            Tag::List(start) => EntityKind::List(start),
            Tag::CodeBlock(lang) => EntityKind::TelegramEntityKind(MessageEntityKind::Pre {
                language: lang.map(|lang| lang.to_string()),
            }),
            Tag::Italic => EntityKind::TelegramEntityKind(MessageEntityKind::Italic),
            Tag::Bold => EntityKind::TelegramEntityKind(MessageEntityKind::Bold),
            Tag::Strikethrough => EntityKind::TelegramEntityKind(MessageEntityKind::Strikethrough),
            Tag::Link(url) | Tag::Image(url) => {
                EntityKind::TelegramEntityKind(MessageEntityKind::TextLink {
                    url: url.parse().unwrap(),
                })
            }
            _ => {
                return Err(anyhow!("Unexpected tag: {:?}", value));
            }
        };
        Ok(mapped)
    }
}

#[derive(Debug)]
struct Entity {
    kind: EntityKind,
    start: usize,
}

const PARAGRAPH_MARGIN: usize = 2;
const LIST_ITEM_MARGIN: usize = 1;

#[derive(Debug)]
struct ParseState {
    entity_stack: Vec<Entity>,
    parsed_string: ParsedString,
    utf16_offset: usize,
    prev_block_margin: usize,
}

impl ParseState {
    fn new() -> Self {
        Self {
            entity_stack: Vec::new(),
            parsed_string: ParsedString::default(),
            utf16_offset: 0,
            prev_block_margin: 0,
        }
    }

    fn close(self) -> ParsedString {
        let Self {
            mut parsed_string,
            prev_block_margin,
            ..
        } = self;

        // Trim the redundant trailing margins.
        parsed_string
            .content
            .truncate(parsed_string.content.len() - prev_block_margin);

        parsed_string
    }

    fn next_state(mut self, event: Event) -> Self {
        match event {
            Event::Start(tag) => self.start(tag),
            Event::End(tag) => self.end(tag),
            Event::Text(text) => self.text(text),
            Event::Code(text) => self.code(text),
            Event::Break => self.r#break(),
        };
        self
    }

    fn start(&mut self, tag: Tag) {
        match tag {
            Tag::Paragraph => {}
            Tag::Heading(level) => {
                self.push_str(&format!("{} ", "#".repeat(level as _)));
            }
            Tag::Item => {
                let item_marker = self
                    .entity_stack
                    .last()
                    .and_then(|entity| match entity.kind {
                        EntityKind::List(Some(start)) => Some(format!("{}. ", start)),
                        EntityKind::List(None) => Some("• ".to_owned()),
                        _ => None,
                    })
                    .expect("Expected a list entity");
                self.push_str(&item_marker);
            }
            _ => {
                let entity_kind = tag.try_into().expect("Unexpected tag");
                self.entity_stack.push(Entity {
                    kind: entity_kind,
                    start: self.utf16_offset,
                });
            }
        }
    }

    fn end(&mut self, tag: Tag) {
        match tag {
            Tag::Paragraph | Tag::Heading(_) => {
                self.push_block(PARAGRAPH_MARGIN);
            }
            Tag::CodeBlock(_) => {
                let Entity { kind, start } = self.entity_stack.pop().expect("Unmatched end tag");
                self.parsed_string.entities.push(MessageEntity {
                    kind: if let EntityKind::TelegramEntityKind(kind) = kind {
                        kind
                    } else {
                        panic!("Unexpected entity kind: {:?}", kind)
                    },
                    offset: start,
                    length: self.utf16_offset - start,
                });
                if self.parsed_string.content.ends_with('\n') {
                    // Usually, there will be a newline in the end of the code block.
                    // We want to take it into consideration when performing collapsing.
                    self.prev_block_margin = 1;
                }
                self.push_block(PARAGRAPH_MARGIN);
            }
            Tag::List(_) => {
                self.entity_stack.pop().expect("Unmatched end tag");
                self.push_block(PARAGRAPH_MARGIN);
            }
            Tag::Item => {
                if let Some(Entity {
                    kind: EntityKind::List(maybe_start_number),
                    ..
                }) = self.entity_stack.last_mut()
                {
                    if let Some(start_number) = maybe_start_number {
                        *start_number += 1;
                    }
                } else {
                    panic!("Unmatched end tag");
                }
                self.push_block(LIST_ITEM_MARGIN)
            }
            Tag::Italic | Tag::Bold | Tag::Strikethrough => {
                let Entity { kind, start } = self.entity_stack.pop().expect("Unmatched end tag");
                self.parsed_string.entities.push(MessageEntity {
                    kind: if let EntityKind::TelegramEntityKind(kind) = kind {
                        kind
                    } else {
                        panic!("Unexpected entity kind: {:?}", kind)
                    },
                    offset: start,
                    length: self.utf16_offset - start,
                });
            }
            Tag::Link(_) | Tag::Image(_) => {
                if let Some(Entity {
                    kind: EntityKind::TelegramEntityKind(kind),
                    start,
                }) = self.entity_stack.pop()
                {
                    self.parsed_string.entities.push(MessageEntity {
                        kind,
                        offset: start,
                        length: self.utf16_offset - start,
                    });
                } else {
                    panic!("Unmatched end tag");
                }
            }
        }
    }

    fn text(&mut self, text: CowStr) {
        self.push_str(&text);
    }

    fn code(&mut self, text: CowStr) {
        let offset = self.utf16_offset;
        self.push_str(&text);
        self.parsed_string.entities.push(MessageEntity {
            kind: MessageEntityKind::Code,
            offset,
            length: self.utf16_offset - offset,
        });
    }

    fn r#break(&mut self) {
        self.push_str("\n");
    }

    fn push_str(&mut self, string: &str) {
        let utf16_len_inc = string.encode_utf16().count();
        self.parsed_string.content.push_str(string);
        self.utf16_offset += utf16_len_inc;
        self.prev_block_margin = 0;
    }

    fn push_block(&mut self, margin: usize) {
        if self.prev_block_margin >= margin {
            return;
        }

        let this_margin = margin - self.prev_block_margin;
        self.push_str(&"\n".repeat(this_margin));
        self.prev_block_margin = margin;
    }
}

#[allow(unused)]
pub fn parse(content: &str) -> ParsedString {
    let mut options = CmarkOptions::empty();
    options.insert(CmarkOptions::ENABLE_STRIKETHROUGH);
    let parser = CmarkParser::new_ext(content, options);

    parser
        .filter_map(|event| {
            #[cfg(debug_assertions)]
            {
                Some(Event::try_from(event).expect("Unexpected event"))
            }
            #[cfg(not(debug_assertions))]
            {
                Event::try_from(event).ok()
            }
        })
        .fold(ParseState::new(), |acc, event| acc.next_state(event))
        .close()
}

#[cfg(test)]
mod tests {
    use teloxide::types::{MessageEntity, MessageEntityKind};

    use super::*;

    #[test]
    fn my_test() {
        let content = "\n\n```rust\nfn is_prime(num: u64) -> bool {\n    if num <= 1 {\n        return false;\n    }\n    for i in 2..=((num as f64).sqrt() as u64) {\n        if num % i == 0 {\n            return false;\n        }\n    }\n    return true;\n}\n\nfn main() {\n    let num = 17;\n    if is_prime(num) {\n        println!(\"{} is a prime number\", num);\n    } else {\n        println!(\"{} is not a prime number\", num);\n    }\n}\n```\n\n输出：\n\n```\n17 is a prime number\n```";
        let events: Vec<_> = pulldown_cmark::Parser::new(content).collect();
        println!("{:#?}", events);
    }

    #[test]
    fn test_parse_simple() {
        let raw = r#"# Heading
- list item 1
- list item 2

Next Paragraph"#;
        let expected_content = r#"# Heading

• list item 1
• list item 2

Next Paragraph"#;
        let parsed = parse(raw);

        assert_eq!(parsed.content, expected_content);
    }

    #[test]
    fn test_parse_paragraph_list() {
        let raw = r#"- list item 1

- list item 2

- list item 3"#;
        let expected_content = r#"• list item 1

• list item 2

• list item 3"#;
        let parsed = parse(raw);

        assert_eq!(parsed.content, expected_content);
    }

    #[test]
    fn test_code() {
        let raw = r#"This is a code snippet:
```c
printf("hello\n");
```

End"#;
        let expected_content = r#"This is a code snippet:

printf("hello\n");

End"#;
        let parsed = parse(raw);

        assert_eq!(parsed.content, expected_content);
        assert!(matches!(
            parsed.entities[0],
            MessageEntity {
                kind: MessageEntityKind::Pre {
                    language: Some(ref lang)
                },
                offset: 25,
                length: 19
            } if lang == "c"
        ));
    }

    #[test]
    fn test_inline_formats() {
        let raw = r#"this is **bold *bold italic* text**"#;
        let expected_content = r#"this is bold bold italic text"#;
        let parsed = parse(raw);

        println!("{:#?}", parsed);
        assert_eq!(parsed.content, expected_content);
        assert!(matches!(
            parsed.entities[0],
            MessageEntity {
                kind: MessageEntityKind::Italic,
                offset: 13,
                length: 11
            }
        ));
        assert!(matches!(
            parsed.entities[1],
            MessageEntity {
                kind: MessageEntityKind::Bold,
                offset: 8,
                length: 21
            }
        ));
    }
}
