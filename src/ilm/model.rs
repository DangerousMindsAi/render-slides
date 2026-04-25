#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TextAlignment {
    Left,
    Center,
    Right,
    Justify,
}

#[derive(Clone)]
pub(crate) struct IlmTextRun {
    pub(crate) x: i64,
    pub(crate) y: i64,
    pub(crate) cx: i64,
    pub(crate) cy: i64,
    pub(crate) text: String,
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
}

#[derive(Clone)]
pub(crate) struct IlmSlide {
    pub(crate) elements: Vec<IlmElement>,
}
