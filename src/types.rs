use serde::Serialize;

#[derive(Serialize)]
pub(crate) struct SchemaSummary {
    pub(crate) version: &'static str,
    pub(crate) slide_layouts: Vec<&'static str>,
    pub(crate) qualitative_aliases: Vec<&'static str>,
}

#[derive(Serialize)]
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
