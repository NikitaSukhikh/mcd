//! Markdown parsing into the MCD canonical block stream.

use std::collections::HashSet;

use comrak::{
    Arena, Options,
    nodes::{AstNode, NodeCode, NodeMath, NodeValue, Sourcepos},
    parse_document,
};

use crate::{
    directives::{DirectiveParseOptions, parse_image_directive, parse_table_directive},
    document::{DocumentBlock, McdDocument, SourceSpan},
    errors::{Diagnostic, McdError, Result},
};

/// Markdown parsing options.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MarkdownParseOptions {
    /// Reject unknown directive fields.
    pub strict_directives: bool,
}

impl Default for MarkdownParseOptions {
    fn default() -> Self {
        Self {
            strict_directives: true,
        }
    }
}

/// Parse Markdown into the canonical MCD document stream.
pub fn parse_markdown(source_path: &str, markdown: &str) -> Result<McdDocument> {
    parse_markdown_with_options(source_path, markdown, MarkdownParseOptions::default())
}

/// Parse Markdown into the canonical MCD document stream with explicit options.
pub fn parse_markdown_with_options(
    source_path: &str,
    markdown: &str,
    options: MarkdownParseOptions,
) -> Result<McdDocument> {
    let arena = Arena::new();
    let mut comrak_options = Options::default();
    comrak_options.extension.block_directive = true;
    comrak_options.extension.math_dollars = true;
    comrak_options.extension.math_code = true;
    comrak_options.parse.sourcepos_chars = true;

    let root = parse_document(&arena, markdown, &comrak_options);
    let mut parser = MarkdownBlockParser {
        source_path,
        options,
        next_index: 1,
        blocks: Vec::new(),
        placement_refs: HashSet::new(),
    };
    parser.visit_container(root)?;

    Ok(McdDocument {
        source_path: source_path.to_string(),
        blocks: parser.blocks,
    })
}

struct MarkdownBlockParser<'source> {
    source_path: &'source str,
    options: MarkdownParseOptions,
    next_index: usize,
    blocks: Vec<DocumentBlock>,
    placement_refs: HashSet<String>,
}

impl MarkdownBlockParser<'_> {
    fn visit_container<'arena>(&mut self, node: &'arena AstNode<'arena>) -> Result<()> {
        for child in node.children() {
            self.visit_block(child)?;
        }
        Ok(())
    }

    fn visit_block<'arena>(&mut self, node: &'arena AstNode<'arena>) -> Result<()> {
        let data = node.data();
        let source = source_span(data.sourcepos);
        match &data.value {
            NodeValue::Document => self.visit_container(node),
            NodeValue::Heading(heading) => {
                let text = collect_plain_text(node);
                let id = self.next_id("heading");
                self.blocks.push(DocumentBlock::Heading {
                    id,
                    level: heading.level,
                    text,
                    source,
                });
                Ok(())
            }
            NodeValue::Paragraph => {
                if let Some(math) = display_math_text(node) {
                    let id = self.next_id("math_block");
                    self.blocks.push(DocumentBlock::MathBlock {
                        id,
                        text: math,
                        source,
                    });
                } else {
                    let id = self.next_id("paragraph");
                    self.blocks.push(DocumentBlock::Paragraph {
                        id,
                        text: collect_plain_text(node),
                        source,
                    });
                }
                Ok(())
            }
            NodeValue::List(_) => {
                let id = self.next_id("list");
                self.blocks.push(DocumentBlock::List {
                    id,
                    text: collect_plain_text(node),
                    source,
                });
                Ok(())
            }
            NodeValue::CodeBlock(code) => {
                let info = code.info.trim();
                let text = code.literal.clone();
                if info == "math" {
                    let id = self.next_id("math_block");
                    self.blocks
                        .push(DocumentBlock::MathBlock { id, text, source });
                } else {
                    let id = self.next_id("code_block");
                    self.blocks.push(DocumentBlock::CodeBlock {
                        id,
                        language: (!info.is_empty()).then(|| info.to_string()),
                        text,
                        source,
                    });
                }
                Ok(())
            }
            NodeValue::BlockQuote | NodeValue::MultilineBlockQuote(_) => {
                let id = self.next_id("quote");
                self.blocks.push(DocumentBlock::Quote {
                    id,
                    text: collect_plain_text(node),
                    source,
                });
                Ok(())
            }
            NodeValue::BlockDirective(directive) => {
                let info = directive.info.trim();
                match info {
                    "table" => {
                        let body = collect_plain_text(node);
                        let placement = parse_table_directive(
                            &body,
                            source,
                            DirectiveParseOptions {
                                strict: self.options.strict_directives,
                            },
                        )
                        .map_err(|err| self.with_markdown_source(err, source))?;
                        self.validate_placement_ref(placement.ref_id.as_deref(), source)?;
                        let id = self.next_id("table_ref");
                        self.blocks.push(DocumentBlock::TableRef {
                            id,
                            placement,
                            source,
                        });
                        Ok(())
                    }
                    "image" => {
                        let body = collect_plain_text(node);
                        let placement = parse_image_directive(
                            &body,
                            source,
                            DirectiveParseOptions {
                                strict: self.options.strict_directives,
                            },
                        )
                        .map_err(|err| self.with_markdown_source(err, source))?;
                        self.validate_placement_ref(placement.ref_id.as_deref(), source)?;
                        let id = self.next_id("image_ref");
                        self.blocks.push(DocumentBlock::ImageRef {
                            id,
                            placement,
                            source,
                        });
                        Ok(())
                    }
                    "chart" => Err(McdError::from_diagnostic(
                        self.diagnostic(
                            "directive.chart.unsupported",
                            "No separate :::chart directive is supported in MCD 0.1; use :::table with display: chart.",
                            source,
                        ),
                    )),
                    _ => {
                        self.visit_container(node)?;
                        Ok(())
                    }
                }
            }
            NodeValue::Item(_) => self.visit_container(node),
            NodeValue::Table(_) => {
                let id = self.next_id("paragraph");
                self.blocks.push(DocumentBlock::Paragraph {
                    id,
                    text: collect_plain_text(node),
                    source,
                });
                Ok(())
            }
            NodeValue::HtmlBlock(html) => {
                let id = self.next_id("code_block");
                self.blocks.push(DocumentBlock::CodeBlock {
                    id,
                    language: Some("html".to_string()),
                    text: html.literal.clone(),
                    source,
                });
                Ok(())
            }
            NodeValue::ThematicBreak
            | NodeValue::FrontMatter(_)
            | NodeValue::FootnoteDefinition(_)
            | NodeValue::TableRow(_)
            | NodeValue::TableCell
            | NodeValue::DescriptionList
            | NodeValue::DescriptionItem(_)
            | NodeValue::DescriptionTerm
            | NodeValue::DescriptionDetails
            | NodeValue::TaskItem(_)
            | NodeValue::Alert(_)
            | NodeValue::Subtext => self.visit_container(node),
            _ => Ok(()),
        }
    }

    fn validate_placement_ref(
        &mut self,
        placement_ref: Option<&str>,
        source: Option<SourceSpan>,
    ) -> Result<()> {
        let Some(placement_ref) = placement_ref else {
            return Ok(());
        };
        if self.placement_refs.insert(placement_ref.to_string()) {
            Ok(())
        } else {
            Err(McdError::from_diagnostic(self.diagnostic(
                "directive.ref.duplicate",
                format!("Duplicate placement ref '{placement_ref}'."),
                source,
            )))
        }
    }

    fn next_id(&mut self, kind: &str) -> String {
        let id = format!("block-{:04}-{kind}", self.next_index);
        self.next_index += 1;
        id
    }

    fn diagnostic(
        &self,
        code: impl Into<String>,
        message: impl Into<String>,
        source: Option<SourceSpan>,
    ) -> Diagnostic {
        let source = source.map(|span| format!("{}:{span}", self.source_path));
        let mut diagnostic = Diagnostic::error(code, message);
        if let Some(source) = source {
            diagnostic = diagnostic.with_source(source);
        }
        diagnostic
    }

    fn with_markdown_source(&self, err: McdError, source: Option<SourceSpan>) -> McdError {
        let existing = match err.diagnostic() {
            Some(diagnostic) => diagnostic.clone(),
            None => return err,
        };
        let Some(source) = source else {
            return err;
        };
        let mut diagnostic = existing;
        diagnostic.source = Some(format!("{}:{source}", self.source_path));
        McdError::from_diagnostic(diagnostic)
    }
}

fn source_span(sourcepos: Sourcepos) -> Option<SourceSpan> {
    (sourcepos.start.line > 0).then_some(SourceSpan {
        start_line: sourcepos.start.line,
        start_column: sourcepos.start.column,
        end_line: sourcepos.end.line,
        end_column: sourcepos.end.column,
    })
}

fn display_math_text<'arena>(node: &'arena AstNode<'arena>) -> Option<String> {
    let mut non_break_children = node.children().filter(|child| {
        !matches!(
            child.data().value,
            NodeValue::SoftBreak | NodeValue::LineBreak
        )
    });
    let only = non_break_children.next()?;
    if non_break_children.next().is_some() {
        return None;
    }
    match &only.data().value {
        NodeValue::Math(NodeMath {
            display_math: true,
            literal,
            ..
        }) => Some(literal.clone()),
        _ => None,
    }
}

fn collect_plain_text<'arena>(node: &'arena AstNode<'arena>) -> String {
    let mut text = String::new();
    collect_plain_text_inner(node, &mut text);
    normalize_collected_text(&text)
}

fn collect_plain_text_inner<'arena>(node: &'arena AstNode<'arena>, text: &mut String) {
    match &node.data().value {
        NodeValue::Text(value) => text.push_str(value),
        NodeValue::Code(NodeCode { literal, .. }) => text.push_str(literal),
        NodeValue::HtmlInline(value) => text.push_str(value),
        NodeValue::Math(NodeMath { literal, .. }) => text.push_str(literal),
        NodeValue::SoftBreak | NodeValue::LineBreak => text.push('\n'),
        NodeValue::CodeBlock(code) => text.push_str(&code.literal),
        NodeValue::HtmlBlock(html) => text.push_str(&html.literal),
        _ => {
            for child in node.children() {
                collect_plain_text_inner(child, text);
                if child.data().value.block() && !text.ends_with('\n') {
                    text.push('\n');
                }
            }
        }
    }
}

fn normalize_collected_text(text: &str) -> String {
    text.lines()
        .map(str::trim_end)
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{directives::TableDisplay, document::DocumentBlock};

    #[test]
    fn emits_blocks_in_source_order_with_directives() {
        let doc = parse_markdown(
            "content/main.md",
            "# Revenue\n\nIntro.\n\n:::table\nref: revenue-chart\ntable: revenue\nview: quarterly-bar-chart\ndisplay: chart\ncaption: Revenue by quarter\n:::\n\n:::image\nref: process-diagram\nasset: process-diagram\nalt: Diagram\n:::\n",
        )
        .expect("markdown parses");

        assert_eq!(doc.blocks.len(), 4);
        assert!(matches!(doc.blocks[0], DocumentBlock::Heading { .. }));
        assert!(matches!(doc.blocks[1], DocumentBlock::Paragraph { .. }));
        match &doc.blocks[2] {
            DocumentBlock::TableRef { placement, .. } => {
                assert_eq!(placement.ref_id.as_deref(), Some("revenue-chart"));
                assert_eq!(placement.display, TableDisplay::Chart);
            }
            other => panic!("expected table ref, got {other:?}"),
        }
        assert!(matches!(doc.blocks[3], DocumentBlock::ImageRef { .. }));
    }

    #[test]
    fn duplicate_placement_refs_fail() {
        let err = parse_markdown(
            "content/main.md",
            ":::table\nref: repeated\ntable: revenue\n:::\n\n:::image\nref: repeated\nasset: diagram\n:::\n",
        )
        .expect_err("duplicate refs should fail");

        assert_eq!(
            err.diagnostic().map(|d| d.code.as_str()),
            Some("directive.ref.duplicate")
        );
    }

    #[test]
    fn chart_directive_is_not_supported() {
        let err = parse_markdown(
            "content/main.md",
            ":::chart\nref: revenue-chart\ntable: revenue\n:::\n",
        )
        .expect_err("chart directive should fail");

        assert_eq!(
            err.diagnostic().map(|d| d.code.as_str()),
            Some("directive.chart.unsupported")
        );
    }
}
