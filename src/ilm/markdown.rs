use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ListType {
    Unordered,
    Ordered(u64),
}

#[derive(Debug, Clone)]
pub(crate) struct RichRun {
    pub(crate) text: String,
    pub(crate) bold: bool,
    pub(crate) italic: bool,
    pub(crate) strikethrough: bool,
    pub(crate) is_code: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct RichParagraph {
    pub(crate) runs: Vec<RichRun>,
    pub(crate) list_level: u8,
    pub(crate) list_type: Option<ListType>,
    pub(crate) is_quote: bool,
    pub(crate) is_code_block: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct RichTable {
    // Placeholder for future table support
    // (Tables are explicitly not implemented in this pass, but the AST allows it)
}

#[derive(Debug, Clone)]
pub(crate) enum RichBlock {
    Paragraph(RichParagraph),
    Table(RichTable), // For future use
}

pub(crate) fn parse_markdown(text: &str) -> Vec<RichBlock> {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_STRIKETHROUGH);

    let parser = Parser::new_ext(text, options);

    let mut blocks = Vec::new();

    // State
    let mut current_para: Option<RichParagraph> = None;
    let mut list_stack: Vec<(ListType, u64)> = Vec::new();
    let mut is_quote = false;
    let mut is_code_block = false;

    // Inline formatting state
    let mut bold = false;
    let mut italic = false;
    let mut strikethrough = false;
    let is_code = false;

    // Helper to start a paragraph if needed (e.g. for tight lists)
    let ensure_para = |current_para: &mut Option<RichParagraph>, list_stack: &Vec<(ListType, u64)>, is_quote: bool, is_code_block: bool| {
        if current_para.is_none() {
            let (list_type, _idx) = list_stack.last().cloned().unwrap_or((ListType::Unordered, 0));
            let list_type_opt = if !list_stack.is_empty() { Some(list_type) } else { None };
            *current_para = Some(RichParagraph {
                runs: Vec::new(),
                list_level: list_stack.len() as u8,
                list_type: list_type_opt,
                is_quote,
                is_code_block,
            });
        }
    };

    for event in parser {
        match event {
            Event::Start(tag) => match tag {
                Tag::Paragraph => {
                    if let Some(mut p) = current_para.take() {
                        if let Some(last_run) = p.runs.last_mut() {
                            if last_run.text.ends_with('\n') {
                                last_run.text.pop();
                            }
                        }
                        blocks.push(RichBlock::Paragraph(p));
                    }
                    ensure_para(&mut current_para, &list_stack, is_quote, is_code_block);
                }
                Tag::Heading { .. } => {
                    if let Some(mut p) = current_para.take() {
                        if let Some(last_run) = p.runs.last_mut() {
                            if last_run.text.ends_with('\n') {
                                last_run.text.pop();
                            }
                        }
                        blocks.push(RichBlock::Paragraph(p));
                    }
                    ensure_para(&mut current_para, &list_stack, is_quote, is_code_block);
                    bold = true; // Simple approximation for headings for now
                }
                Tag::BlockQuote(_) => {
                    is_quote = true;
                }
                Tag::CodeBlock(_) => {
                    if let Some(mut p) = current_para.take() {
                        if let Some(last_run) = p.runs.last_mut() {
                            if last_run.text.ends_with('\n') {
                                last_run.text.pop();
                            }
                        }
                        blocks.push(RichBlock::Paragraph(p));
                    }
                    is_code_block = true;
                    ensure_para(&mut current_para, &list_stack, is_quote, is_code_block);
                }
                Tag::List(start) => {
                    let l_type = if let Some(n) = start {
                        ListType::Ordered(n)
                    } else {
                        ListType::Unordered
                    };
                    list_stack.push((l_type, start.unwrap_or(1)));
                }
                Tag::Item => {
                    // Each item can have multiple paragraphs. The first paragraph or tight text will use this state.
                    // We don't start a paragraph immediately, we wait for Tag::Paragraph or Event::Text.
                }
                Tag::Strong => bold = true,
                Tag::Emphasis => italic = true,
                Tag::Strikethrough => strikethrough = true,
                _ => {} // Ignore tables for now, fallback gracefully
            },
            Event::End(tag) => match tag {
                TagEnd::Paragraph | TagEnd::Heading(..) | TagEnd::CodeBlock => {
                    if let Some(mut p) = current_para.take() {
                        if let Some(last_run) = p.runs.last_mut() {
                            if last_run.text.ends_with('\n') {
                                last_run.text.pop();
                            }
                        }
                        blocks.push(RichBlock::Paragraph(p));
                    }
                    if matches!(tag, TagEnd::Heading(..)) {
                        bold = false;
                    }
                    is_code_block = false;
                }
                TagEnd::BlockQuote(_) => {
                    is_quote = false;
                }
                TagEnd::List(_) => {
                    list_stack.pop();
                }
                TagEnd::Item => {
                    // If a list item ended but there's still a pending para (e.g. tight list text), flush it
                    if let Some(mut p) = current_para.take() {
                        if let Some(last_run) = p.runs.last_mut() {
                            if last_run.text.ends_with('\n') {
                                last_run.text.pop();
                            }
                        }
                        blocks.push(RichBlock::Paragraph(p));
                    }
                    // Increment the counter for the current list if it's ordered
                    if let Some((ListType::Ordered(n), _)) = list_stack.last_mut() {
                        *n += 1;
                    }
                }
                TagEnd::Strong => bold = false,
                TagEnd::Emphasis => italic = false,
                TagEnd::Strikethrough => strikethrough = false,
                _ => {}
            },
            Event::Text(text) => {
                ensure_para(&mut current_para, &list_stack, is_quote, is_code_block);
                if let Some(p) = &mut current_para {
                    p.runs.push(RichRun {
                        text: text.to_string(),
                        bold,
                        italic,
                        strikethrough,
                        is_code,
                    });
                }
            }
            Event::Code(text) => {
                ensure_para(&mut current_para, &list_stack, is_quote, is_code_block);
                if let Some(p) = &mut current_para {
                    p.runs.push(RichRun {
                        text: text.to_string(),
                        bold,
                        italic,
                        strikethrough,
                        is_code: true,
                    });
                }
            }
            Event::SoftBreak | Event::HardBreak => {
                if let Some(p) = &mut current_para {
                    p.runs.push(RichRun {
                        text: "\n".to_string(),
                        bold: false,
                        italic: false,
                        strikethrough: false,
                        is_code: false,
                    });
                }
            }
            _ => {}
        }
    }

    // Flush any remaining paragraph
    if let Some(mut p) = current_para.take() {
        if let Some(last_run) = p.runs.last_mut() {
            if last_run.text.ends_with('\n') {
                last_run.text.pop();
            }
        }
        blocks.push(RichBlock::Paragraph(p));
    }

    blocks
}
