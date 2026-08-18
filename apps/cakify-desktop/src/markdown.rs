use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MarkdownBlock {
    Paragraph(String),
    Heading { level: u8, text: String },
    Code { language: String, text: String },
    Quote(String),
    ListItem { ordered: bool, text: String },
    Rule,
}

enum PendingKind {
    Paragraph,
    Heading(u8),
    Code(String),
    Quote,
    ListItem { ordered: bool },
}

struct PendingBlock {
    kind: PendingKind,
    text: String,
}

pub fn parse_markdown(source: &str) -> Vec<MarkdownBlock> {
    let options = Options::ENABLE_TABLES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_CJK_FRIENDLY_EMPHASIS;
    let mut blocks = Vec::new();
    let mut stack = Vec::<PendingBlock>::new();
    let mut list_ordered = Vec::<bool>::new();

    for event in Parser::new_ext(source, options) {
        match event {
            Event::Start(Tag::Paragraph) => {
                if !matches!(
                    stack.last().map(|block| &block.kind),
                    Some(PendingKind::ListItem { .. } | PendingKind::Quote)
                ) {
                    stack.push(PendingBlock {
                        kind: PendingKind::Paragraph,
                        text: String::new(),
                    });
                }
            }
            Event::Start(Tag::Heading { level, .. }) => stack.push(PendingBlock {
                kind: PendingKind::Heading(heading_level(level)),
                text: String::new(),
            }),
            Event::Start(Tag::CodeBlock(kind)) => stack.push(PendingBlock {
                kind: PendingKind::Code(match kind {
                    CodeBlockKind::Indented => String::new(),
                    CodeBlockKind::Fenced(language) => language.to_string(),
                }),
                text: String::new(),
            }),
            Event::Start(Tag::BlockQuote(_)) => stack.push(PendingBlock {
                kind: PendingKind::Quote,
                text: String::new(),
            }),
            Event::Start(Tag::List(start)) => list_ordered.push(start.is_some()),
            Event::Start(Tag::Item) => stack.push(PendingBlock {
                kind: PendingKind::ListItem {
                    ordered: list_ordered.last().copied().unwrap_or(false),
                },
                text: String::new(),
            }),
            Event::Text(text) => append_text(&mut stack, &text),
            Event::Code(code) => {
                append_text(&mut stack, "`");
                append_text(&mut stack, &code);
                append_text(&mut stack, "`");
            }
            Event::InlineMath(math) => {
                append_text(&mut stack, "$");
                append_text(&mut stack, &math);
                append_text(&mut stack, "$");
            }
            Event::DisplayMath(math) => append_text(&mut stack, &math),
            Event::SoftBreak | Event::HardBreak => append_text(&mut stack, "\n"),
            Event::TaskListMarker(checked) => {
                append_text(&mut stack, if checked { "[x] " } else { "[ ] " });
            }
            Event::Rule => blocks.push(MarkdownBlock::Rule),
            Event::End(TagEnd::Paragraph) => {
                if matches!(
                    stack.last().map(|block| &block.kind),
                    Some(PendingKind::Paragraph)
                ) {
                    finish_block(&mut stack, &mut blocks);
                }
            }
            Event::End(TagEnd::Heading(_))
            | Event::End(TagEnd::CodeBlock)
            | Event::End(TagEnd::BlockQuote(_))
            | Event::End(TagEnd::Item) => finish_block(&mut stack, &mut blocks),
            Event::End(TagEnd::List(_)) => {
                list_ordered.pop();
            }
            Event::Start(_)
            | Event::End(_)
            | Event::Html(_)
            | Event::InlineHtml(_)
            | Event::FootnoteReference(_) => {}
        }
    }

    while !stack.is_empty() {
        finish_block(&mut stack, &mut blocks);
    }
    if blocks.is_empty() && !source.is_empty() {
        blocks.push(MarkdownBlock::Paragraph(source.to_owned()));
    }
    blocks
}

fn append_text(stack: &mut [PendingBlock], text: &str) {
    if let Some(block) = stack.last_mut() {
        block.text.push_str(text);
    }
}

fn finish_block(stack: &mut Vec<PendingBlock>, blocks: &mut Vec<MarkdownBlock>) {
    let Some(block) = stack.pop() else {
        return;
    };
    let text = block.text.trim_end().to_owned();
    if text.is_empty() && !matches!(block.kind, PendingKind::Code(_)) {
        return;
    }
    let block = match block.kind {
        PendingKind::Paragraph => MarkdownBlock::Paragraph(text),
        PendingKind::Heading(level) => MarkdownBlock::Heading { level, text },
        PendingKind::Code(language) => MarkdownBlock::Code {
            language,
            text: block.text,
        },
        PendingKind::Quote => MarkdownBlock::Quote(text),
        PendingKind::ListItem { ordered } => MarkdownBlock::ListItem { ordered, text },
    };
    blocks.push(block);
}

fn heading_level(level: HeadingLevel) -> u8 {
    match level {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_safe_common_blocks_without_exposing_html() {
        let blocks = parse_markdown(
            "# 标题\n\n正文 **加粗**\n\n```rust\nfn main() {}\n```\n\n<script>x</script>",
        );
        assert!(matches!(
            &blocks[0],
            MarkdownBlock::Heading { level: 1, text } if text == "标题"
        ));
        assert!(blocks.iter().any(|block| matches!(
            block,
            MarkdownBlock::Code { language, text }
                if language == "rust" && text.contains("fn main")
        )));
        assert!(!blocks.iter().any(|block| match block {
            MarkdownBlock::Paragraph(text)
            | MarkdownBlock::Quote(text)
            | MarkdownBlock::ListItem { text, .. }
            | MarkdownBlock::Heading { text, .. }
            | MarkdownBlock::Code { text, .. } => text.contains("script"),
            MarkdownBlock::Rule => false,
        }));
    }
}
