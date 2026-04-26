use serde::Serialize;

#[derive(Serialize)]
pub(crate) struct LayoutSpec {
    pub(crate) name: &'static str,
    pub(crate) description: &'static str,
    pub(crate) required_slots: Vec<&'static str>,
    pub(crate) optional_slots: Vec<&'static str>,
}

#[derive(Serialize)]
pub(crate) struct LayoutsSummary {
    pub(crate) version: &'static str,
    pub(crate) slide_layouts: Vec<LayoutSpec>,
}

#[derive(Serialize)]
pub(crate) struct TweakInstructions {
    pub(crate) qualitative_tweaks: Vec<OperationSpec>,
    pub(crate) quantitative_tweaks: Vec<OperationSpec>,
    pub(crate) structural_operations: Vec<OperationSpec>,
}

#[derive(Serialize, Clone)]
pub(crate) struct OperationSpec {
    pub(crate) name: &'static str,
    pub(crate) description: &'static str,
    pub(crate) params: Vec<&'static str>,
    pub(crate) bounds: &'static str,
}

#[derive(Serialize)]
pub(crate) struct OperationExplanation {
    pub(crate) path: String,
    pub(crate) operation: String,
    pub(crate) semantics: &'static str,
    pub(crate) side_effects: Vec<&'static str>,
    pub(crate) constraints: Vec<&'static str>,
}

#[derive(Serialize)]
pub(crate) struct OperationExample {
    pub(crate) request: &'static str,
    pub(crate) effect: &'static str,
}
