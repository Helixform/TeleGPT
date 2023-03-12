use anyhow::Ok;
use pulldown_cmark::{
    CowStr, Event as CmarkEvent, Options as CmarkOptions, Parser as CmarkParser, Tag as CmarkTag,
};
use teloxide::types::{MessageEntity, MessageEntityKind};

#[derive(Debug)]
pub struct ParsedString {
    pub content: String,
    pub entities: Vec<MessageEntity>,
}

impl ParsedString {
    fn new() -> Self {
        Self {
            content: String::new(),
            entities: Vec::new(),
        }
    }

    fn trimmed(mut self) -> Self {
        self.content.truncate(self.content.trim().len());
        self
    }
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
    CodeBlock,
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
            CmarkTag::CodeBlock(_) => Tag::CodeBlock,
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
            Tag::CodeBlock => EntityKind::TelegramEntityKind(MessageEntityKind::Code),
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

#[derive(Debug)]
struct ParseState {
    entity_stack: Vec<Entity>,
    parsed_string: ParsedString,
    utf16_offset: usize,
}

impl ParseState {
    fn new() -> Self {
        Self {
            entity_stack: Vec::new(),
            parsed_string: ParsedString::new(),
            utf16_offset: 0,
        }
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
            Tag::Paragraph => {
                self.push_str("\n");
            }
            Tag::Heading(_) => self.push_str("\n"),
            Tag::CodeBlock => {
                let entity = self.entity_stack.pop().expect("Unmatched end tag");
                self.parsed_string.entities.push(MessageEntity {
                    kind: MessageEntityKind::Code,
                    offset: entity.start,
                    length: self.utf16_offset - entity.start,
                });
                self.push_str("\n");
            }
            Tag::List(_) => {
                self.entity_stack.pop().expect("Unmatched end tag");
                self.push_str("\n");
            }
            Tag::Item => {
                self.push_str("\n");
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
        .parsed_string
        .trimmed()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse() {
        let content = r#"
很抱歉，我理解有误。以下是将 `title` 属性添加到 Markdown 格式的图片标题中的无序列表：

- 苹果 ![苹果](https://img.icons8.com/color/48/000000/apple.png)
- 香蕉 ![香蕉](https://img.icons8.com/color/48/000000/banana.png)
- 草莓 ![草莓](https://img.icons8.com/color/48/000000/strawberry.png)
- 橙子 ![橙子](https://img.icons8.com/color/48/000000/orange.png)
- 葡萄 ![葡萄](https://img.icons8.com/color/48/000000/grapes.png)

```rust
fn main() {

}
```

这是一个**加粗**文本
"#;
        let parsed = parse(content);
        println!("{:#?}", parsed);
    }
}
