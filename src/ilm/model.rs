#[derive(Clone)]
pub(crate) struct IlmTextRun {
    pub(crate) x: i64,
    pub(crate) y: i64,
    pub(crate) cx: i64,
    pub(crate) cy: i64,
    pub(crate) text: String,
    pub(crate) font_size_pt: i64,
    pub(crate) bold: bool,
}

#[derive(Clone)]
pub(crate) struct IlmImage {
    pub(crate) x: i64,
    pub(crate) y: i64,
    pub(crate) cx: i64,
    pub(crate) cy: i64,
    pub(crate) uri: String,
}

#[derive(Clone)]
pub(crate) struct IlmSlide {
    pub(crate) text_runs: Vec<IlmTextRun>,
    pub(crate) image: Option<IlmImage>,
}
