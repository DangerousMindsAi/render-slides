#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TextAlignment {
    Left,
    Center,
    Right,
    Justify,
}

use crate::ilm::markdown::RichBlock;

#[derive(Clone, Debug)]
pub(crate) struct IlmTextRun {
    pub(crate) x: i64,
    pub(crate) y: i64,
    pub(crate) cx: i64,
    pub(crate) cy: i64,
    pub(crate) blocks: Vec<RichBlock>,
    pub(crate) font_size_pt: i64,
    pub(crate) bold: bool,
    pub(crate) alignment: TextAlignment,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ImageScaling {
    Stretch,
    Contain,
    Cover,
    FitWidth,
    FitHeight,
}

#[derive(Clone)]
pub(crate) struct IlmImage {
    pub(crate) x: i64,
    pub(crate) y: i64,
    pub(crate) cx: i64,
    pub(crate) cy: i64,
    pub(crate) uri: String,
    pub(crate) image_data: Vec<u8>,
    pub(crate) scaling: ImageScaling,
}

#[derive(Clone)]
pub(crate) enum IlmElement {
    Text(IlmTextRun),
    Image(IlmImage),
    Table(IlmTable),
}

#[derive(Debug, Clone)]
pub(crate) struct IlmTableCell {
    pub(crate) text: IlmTextRun,
    pub(crate) alignment: TextAlignment,
}

#[derive(Debug, Clone)]
pub(crate) struct IlmTableRow {
    pub(crate) cells: Vec<IlmTableCell>,
    pub(crate) is_header: bool,
    pub(crate) row_height_emu: i64,
}

#[derive(Debug, Clone)]
pub(crate) struct IlmTable {
    pub(crate) x: i64,
    pub(crate) y: i64,
    pub(crate) cx: i64,
    pub(crate) cy: i64,
    pub(crate) rows: Vec<IlmTableRow>,
    pub(crate) col_widths_emu: Vec<i64>,
}

#[derive(Clone)]
pub(crate) struct IlmSlide {
    pub(crate) elements: Vec<IlmElement>,
}
