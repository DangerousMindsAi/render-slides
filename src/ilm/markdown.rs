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

use crate::ilm::model::TextAlignment;

#[derive(Debug, Clone)]
pub(crate) struct RichTableCell {
    pub(crate) paragraphs: Vec<RichParagraph>,
    pub(crate) alignment: TextAlignment,
}

#[derive(Debug, Clone)]
pub(crate) struct RichTableRow {
    pub(crate) cells: Vec<RichTableCell>,
    pub(crate) is_header: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct RichTable {
    pub(crate) rows: Vec<RichTableRow>,
    pub(crate) column_alignments: Vec<TextAlignment>,
}

#[derive(Debug, Clone)]
pub(crate) enum RichBlock {
    Paragraph(RichParagraph),
    Table(RichTable),
}

pub(crate) fn parse_markdown(text: &str) -> Vec<RichBlock> {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_STRIKETHROUGH);

    let parser = Parser::new_ext(text, options);

    let mut blocks = Vec::new();

    // Table State
    let mut current_table: Option<RichTable> = None;
    let mut current_row: Option<RichTableRow> = None;
    let mut current_cell: Option<RichTableCell> = None;
    let mut is_table_header = false;
    let mut cell_idx = 0;

    macro_rules! push_paragraph {
        ($p:expr) => {
            if let Some(cell) = &mut current_cell {
                cell.paragraphs.push($p);
            } else {
                blocks.push(RichBlock::Paragraph($p));
            }
        };
    }

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
                        push_paragraph!(p);
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
                        push_paragraph!(p);
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
                        push_paragraph!(p);
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
                Tag::Table(alignments) => {
                    let column_alignments = alignments.into_iter().map(|a| match a {
                        pulldown_cmark::Alignment::None => TextAlignment::Left,
                        pulldown_cmark::Alignment::Left => TextAlignment::Left,
                        pulldown_cmark::Alignment::Center => TextAlignment::Center,
                        pulldown_cmark::Alignment::Right => TextAlignment::Right,
                    }).collect();
                    current_table = Some(RichTable { rows: Vec::new(), column_alignments });
                }
                Tag::TableHead => {
                    is_table_header = true;
                }
                Tag::TableRow => {
                    current_row = Some(RichTableRow { cells: Vec::new(), is_header: is_table_header });
                    cell_idx = 0;
                }
                Tag::TableCell => {
                    let alignment = if let Some(table) = &current_table {
                        table.column_alignments.get(cell_idx).cloned().unwrap_or(TextAlignment::Left)
                    } else {
                        TextAlignment::Left
                    };
                    current_cell = Some(RichTableCell { paragraphs: Vec::new(), alignment });
                }
                _ => {}
            },
            Event::End(tag) => match tag {
                TagEnd::Paragraph | TagEnd::Heading(..) | TagEnd::CodeBlock => {
                    if let Some(mut p) = current_para.take() {
                        if let Some(last_run) = p.runs.last_mut() {
                            if last_run.text.ends_with('\n') {
                                last_run.text.pop();
                            }
                        }
                        push_paragraph!(p);
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
                        push_paragraph!(p);
                    }
                    // Increment the counter for the current list if it's ordered
                    if let Some((ListType::Ordered(n), _)) = list_stack.last_mut() {
                        *n += 1;
                    }
                }
                TagEnd::Strong => bold = false,
                TagEnd::Emphasis => italic = false,
                TagEnd::Strikethrough => strikethrough = false,
                TagEnd::Table => {
                    if let Some(table) = current_table.take() {
                        blocks.push(RichBlock::Table(table));
                    }
                }
                TagEnd::TableHead => {
                    is_table_header = false;
                }
                TagEnd::TableRow => {
                    if let (Some(mut table), Some(row)) = (current_table.as_mut(), current_row.take()) {
                        table.rows.push(row);
                    }
                }
                TagEnd::TableCell => {
                    if let Some(mut p) = current_para.take() {
                        if let Some(last_run) = p.runs.last_mut() {
                            if last_run.text.ends_with('\n') {
                                last_run.text.pop();
                            }
                        }
                        push_paragraph!(p);
                    }
                    if let (Some(mut row), Some(cell)) = (current_row.as_mut(), current_cell.take()) {
                        row.cells.push(cell);
                    }
                    cell_idx += 1;
                }
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
        push_paragraph!(p);
    }

    blocks
}
