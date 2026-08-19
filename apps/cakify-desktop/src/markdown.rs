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
    TableCell,
}

struct PendingBlock {
    kind: PendingKind,
    text: String,
}

pub fn parse_markdown(source: &str) -> Vec<MarkdownBlock> {
    let options = Options::ENABLE_TABLES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_FOOTNOTES
        | Options::ENABLE_MATH;
    let mut blocks = Vec::new();
    let mut stack = Vec::<PendingBlock>::new();
    let mut list_ordered = Vec::<bool>::new();
    let mut table_row = None::<Vec<String>>;
    let mut visible_links = Vec::<Option<String>>::new();

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
            Event::Start(Tag::TableHead) | Event::Start(Tag::TableRow) => {
                table_row = Some(Vec::new());
            }
            Event::Start(Tag::TableCell) => stack.push(PendingBlock {
                kind: PendingKind::TableCell,
                text: String::new(),
            }),
            Event::Start(Tag::Link { dest_url, .. }) => {
                visible_links.push(safe_visible_link(&dest_url));
            }
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
            Event::FootnoteReference(label) => {
                append_text(&mut stack, "[^");
                append_text(&mut stack, &label);
                append_text(&mut stack, "]");
            }
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
            Event::End(TagEnd::TableCell) => {
                if let Some(cell) = stack.pop() {
                    if matches!(cell.kind, PendingKind::TableCell) {
                        table_row
                            .get_or_insert_default()
                            .push(cell.text.trim().to_owned());
                    }
                }
            }
            Event::End(TagEnd::TableHead) | Event::End(TagEnd::TableRow) => {
                finish_table_row(&mut table_row, &mut blocks);
            }
            Event::End(TagEnd::Link) => {
                if let Some(Some(destination)) = visible_links.pop() {
                    append_text(&mut stack, " (");
                    append_text(&mut stack, &destination);
                    append_text(&mut stack, ")");
                }
            }
            Event::Start(_) | Event::End(_) | Event::Html(_) | Event::InlineHtml(_) => {}
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
        PendingKind::TableCell => MarkdownBlock::Paragraph(text),
    };
    blocks.push(block);
}

fn finish_table_row(table_row: &mut Option<Vec<String>>, blocks: &mut Vec<MarkdownBlock>) {
    let Some(cells) = table_row.take() else {
        return;
    };
    if !cells.is_empty() {
        blocks.push(MarkdownBlock::Paragraph(cells.join(" | ")));
    }
}

fn safe_visible_link(destination: &str) -> Option<String> {
    let destination = destination.trim();
    let lowercase = destination.to_ascii_lowercase();
    let safe_scheme = lowercase.starts_with("https://")
        || lowercase.starts_with("http://")
        || lowercase.starts_with("mailto:")
        || destination.starts_with('#')
        || destination.starts_with('/')
        || destination.starts_with("./")
        || destination.starts_with("../");
    (safe_scheme && destination.len() <= 2_048 && !destination.chars().any(char::is_control))
        .then(|| destination.to_owned())
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

    #[test]
    fn keeps_tables_links_tasks_and_footnotes_readable() {
        let blocks = parse_markdown(
            "| A | B |\n|---|---|\n| 1 | 2 |\n\n- [x] done\n\n[docs](https://example.com)[^1]",
        );
        assert!(blocks
            .iter()
            .any(|block| matches!(block, MarkdownBlock::Paragraph(text) if text == "A | B")));
        assert!(blocks
            .iter()
            .any(|block| matches!(block, MarkdownBlock::Paragraph(text) if text == "1 | 2")));
        assert!(blocks.iter().any(|block| matches!(
            block,
            MarkdownBlock::ListItem { ordered: false, text } if text == "[x] done"
        )));
        assert!(blocks.iter().any(|block| matches!(
            block,
            MarkdownBlock::Paragraph(text)
                if text == "docs (https://example.com)[^1]"
        )));
    }

    #[test]
    fn hides_active_link_schemes_and_remote_image_targets() {
        let blocks = parse_markdown(
            "[run](javascript:alert(1)) ![preview](https://example.com/private.png)",
        );
        assert_eq!(
            blocks,
            vec![MarkdownBlock::Paragraph("run preview".to_owned())]
        );
    }
}
