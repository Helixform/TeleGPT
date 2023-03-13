use std::marker::PhantomData;

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

impl ParsedString {
    fn with_str(string: &str) -> Self {
        Self {
            content: string.to_owned(),
            entities: vec![],
        }
    }
}

enum Event<'a> {
    Start(Tag<'a>),
    End(Tag<'a>),
    Text(CowStr<'a>),
    Code(CowStr<'a>),
    Break,
}

#[derive(Clone, Debug)]
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
    type Error = ParserError<'a>;

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
            _ => return Err(ParserError::UnexpectedCmarkTag(value)),
        };
        Ok(mapped)
    }
}

impl<'a> TryFrom<CmarkEvent<'a>> for Event<'a> {
    type Error = ParserError<'a>;

    fn try_from(value: CmarkEvent<'a>) -> Result<Self, Self::Error> {
        let mapped = match value {
            CmarkEvent::Start(tag) => Event::Start(tag.try_into()?),
            CmarkEvent::End(tag) => Event::End(tag.try_into()?),
            CmarkEvent::Text(text) => Event::Text(text),
            CmarkEvent::Html(text) | CmarkEvent::Code(text) => Event::Code(text),
            CmarkEvent::SoftBreak | CmarkEvent::HardBreak => Event::Break,
            CmarkEvent::Rule => Event::Text(CowStr::Borrowed("---")),
            _ => {
                return Err(ParserError::UnexpectedCmarkEvent(value));
            }
        };
        Ok(mapped)
    }
}

#[derive(Clone, Debug)]
enum EntityKind {
    TelegramEntityKind(MessageEntityKind),
    List(Option<u64>),
}

impl<'a> TryFrom<&Tag<'a>> for EntityKind {
    type Error = ParserError<'a>;

    fn try_from(value: &Tag<'a>) -> Result<Self, Self::Error> {
        let mapped = match value {
            Tag::List(start) => EntityKind::List(*start),
            Tag::CodeBlock(lang) => EntityKind::TelegramEntityKind(MessageEntityKind::Pre {
                language: lang.as_ref().map(|lang| lang.to_string()),
            }),
            Tag::Italic => EntityKind::TelegramEntityKind(MessageEntityKind::Italic),
            Tag::Bold => EntityKind::TelegramEntityKind(MessageEntityKind::Bold),
            Tag::Strikethrough => EntityKind::TelegramEntityKind(MessageEntityKind::Strikethrough),
            Tag::Link(url) | Tag::Image(url) => {
                EntityKind::TelegramEntityKind(MessageEntityKind::TextLink {
                    url: url
                        .parse()
                        .map_err(|_| ParserError::InvalidURL(url.clone()))?,
                })
            }
            _ => {
                return Err(ParserError::UnexpectedTag(value.clone()));
            }
        };
        Ok(mapped)
    }
}

#[derive(Clone, Debug)]
struct Entity {
    kind: EntityKind,
    start: usize,
}

const PARAGRAPH_MARGIN: usize = 2;
const LIST_ITEM_MARGIN: usize = 1;

#[derive(Debug)]
enum ParserError<'input> {
    /// Cannot convert the Cmark tag to our tag.
    UnexpectedCmarkTag(CmarkTag<'input>),
    /// Cannot handle the Cmark event.
    UnexpectedCmarkEvent(CmarkEvent<'input>),
    /// Cannot parse the given URL string.
    InvalidURL(CowStr<'input>),
    /// Cannot handle the tag.
    UnexpectedTag(Tag<'input>),
    /// Meet unmatched entity. The first field is the current entity kind,
    /// and the second field is string of the expected kind.
    UnmatchedEntity(Option<EntityKind>, &'static str),
}

type ParserEventResult<'input> = Result<(), ParserError<'input>>;

#[derive(Debug)]
struct ParseState<'p> {
    entity_stack: Vec<Entity>,
    parsed_string: ParsedString,
    utf16_offset: usize,
    prev_block_margin: usize,
    phantom: PhantomData<&'p str>,
}

impl<'p> ParseState<'p> {
    fn new() -> Self {
        Self {
            entity_stack: Vec::new(),
            parsed_string: ParsedString::default(),
            utf16_offset: 0,
            prev_block_margin: 0,
            phantom: PhantomData,
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

    #[allow(clippy::result_large_err)]
    fn next_state<'input: 'p>(mut self, event: Event<'input>) -> Result<Self, ParserError<'input>> {
        match event {
            Event::Start(tag) => self.start(tag)?,
            Event::End(tag) => self.end(tag)?,
            Event::Text(text) => self.text(text),
            Event::Code(text) => self.code(text),
            Event::Break => self.r#break(),
        };
        Ok(self)
    }

    #[allow(clippy::result_large_err)]
    fn start<'input: 'p>(&mut self, tag: Tag<'input>) -> ParserEventResult<'input> {
        match tag {
            Tag::Paragraph => {}
            Tag::Heading(level) => {
                self.push_str(&format!("{} ", "#".repeat(level as _)));
            }
            Tag::Item => {
                let top_entity_kind = self.entity_stack.last().map(|e| &e.kind);
                let item_marker = top_entity_kind
                    .ok_or_else(|| ParserError::UnmatchedEntity(top_entity_kind.cloned(), "List"))
                    .and_then(|kind| match kind {
                        EntityKind::List(Some(start)) => Ok(format!("{}. ", start)),
                        EntityKind::List(None) => Ok("• ".to_owned()),
                        _ => Err(ParserError::UnmatchedEntity(Some(kind.clone()), "List")),
                    })?;
                self.push_str(&item_marker);
            }
            ref tag_ref => {
                let entity_kind = tag_ref
                    .try_into()
                    .map_err(|_| ParserError::UnexpectedTag(tag))?;
                self.entity_stack.push(Entity {
                    kind: entity_kind,
                    start: self.utf16_offset,
                });
            }
        }
        Ok(())
    }

    #[allow(clippy::result_large_err)]
    fn end<'input: 'p>(&mut self, tag: Tag<'input>) -> ParserEventResult<'input> {
        match tag {
            Tag::Paragraph | Tag::Heading(_) => {
                self.push_block(PARAGRAPH_MARGIN);
            }
            Tag::CodeBlock(_) => {
                let Entity { kind, start } = self
                    .entity_stack
                    .pop()
                    .ok_or(ParserError::UnmatchedEntity(None, "Pre"))?;
                self.parsed_string.entities.push(MessageEntity {
                    kind: if let EntityKind::TelegramEntityKind(
                        kind @ MessageEntityKind::Pre { .. },
                    ) = kind
                    {
                        kind
                    } else {
                        return Err(ParserError::UnmatchedEntity(Some(kind), "Pre"));
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
                let Entity { kind, .. } = self
                    .entity_stack
                    .pop()
                    .ok_or(ParserError::UnmatchedEntity(None, "List"))?;
                if let EntityKind::List(_) = kind {
                    self.push_block(PARAGRAPH_MARGIN);
                } else {
                    return Err(ParserError::UnmatchedEntity(Some(kind), "List"));
                }
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
                    return Err(ParserError::UnmatchedEntity(
                        self.entity_stack.last().map(|e| e.kind.clone()),
                        "List",
                    ));
                }
                self.push_block(LIST_ITEM_MARGIN)
            }
            Tag::Italic | Tag::Bold | Tag::Strikethrough => {
                let Entity { kind, start } = self
                    .entity_stack
                    .pop()
                    .ok_or(ParserError::UnmatchedEntity(None, "InlineFormat"))?;
                self.parsed_string.entities.push(MessageEntity {
                    kind: if let EntityKind::TelegramEntityKind(kind) = kind {
                        // FIXME: continue to validate the `MessageEntityKind`.
                        kind
                    } else {
                        return Err(ParserError::UnmatchedEntity(Some(kind), "InlineFormat"));
                    },
                    offset: start,
                    length: self.utf16_offset - start,
                });
            }
            Tag::Link(_) | Tag::Image(_) => {
                let Entity { kind, start } = self
                    .entity_stack
                    .pop()
                    .ok_or(ParserError::UnmatchedEntity(None, "LinkOrImage"))?;

                self.parsed_string.entities.push(MessageEntity {
                    kind: if let EntityKind::TelegramEntityKind(
                        kind @ MessageEntityKind::TextLink { .. },
                    ) = kind
                    {
                        kind
                    } else {
                        return Err(ParserError::UnmatchedEntity(Some(kind), "LinkOrImage"));
                    },
                    offset: start,
                    length: self.utf16_offset - start,
                });
            }
        }
        Ok(())
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
    let mut parser = CmarkParser::new_ext(content, options);

    let result = parser.try_fold(ParseState::new(), |acc, event| {
        let mapped_event = Event::try_from(event)?;
        acc.next_state(mapped_event)
    });

    match result {
        Ok(state) => state.close(),
        Err(err) => {
            error!("Error while parsing Markdown: {:?}", err);
            ParsedString::with_str(content)
        }
    }
}

#[cfg(test)]
mod tests {
    use teloxide::types::{MessageEntity, MessageEntityKind};

    use super::*;

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

    #[test]
    fn test_malformed_url() {
        let raw = r#"This is a [link](invalid)"#;
        let parsed = parse(raw);
        assert_eq!(parsed.content, raw);
    }
}
