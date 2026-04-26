use std::collections::BTreeSet;

use crate::generated;
use crate::types::{OperationExample, OperationExplanation, OperationSpec};

pub(crate) fn all_editable_paths() -> Vec<&'static str> {
    let mut unique = BTreeSet::new();
    unique.extend(generated::TEMPLATE_EDITABLE_PATHS.iter().copied());
    unique.into_iter().collect()
}

pub(crate) fn supports_path(path: &str) -> bool {
    all_editable_paths().contains(&path)
}

pub(crate) fn operation_specs_for(path: &str) -> Option<Vec<OperationSpec>> {
    let from_template: Vec<_> = generated::TEMPLATE_OPERATION_SPECS
        .iter()
        .filter(|entry| entry.path == path)
        .map(|entry| OperationSpec {
            path: None,
            name: entry.name,
            description: entry.description,
            params: entry.params.to_vec(),
            bounds: entry.bounds,
        })
        .collect();

    if from_template.is_empty() {
        None
    } else {
        Some(from_template)
    }
}

pub(crate) fn explain_operation(
    path: &str,
    operation: &str,
) -> Result<OperationExplanation, String> {
    let operations =
        operation_specs_for(path).ok_or_else(|| format!("Unsupported editable path: {path}"))?;

    let op = operations
        .into_iter()
        .find(|op| op.name == operation)
        .ok_or_else(|| format!("Unsupported operation '{operation}' for path '{path}'"))?;

    Ok(OperationExplanation {
        path: path.to_string(),
        operation: operation.to_string(),
        semantics: op.description,
        side_effects: vec![
            "May trigger text reflow inside the resolved layout box.",
            "May require overflow checks before render emitters run.",
        ],
        constraints: vec![op.bounds],
    })
}

pub(crate) fn get_examples(path: &str, operation: &str) -> Result<Vec<OperationExample>, String> {
    if !supports_path(path) {
        return Err(format!("Unsupported editable path: {path}"));
    }

    let supported_operations =
        operation_specs_for(path).ok_or_else(|| format!("Unsupported editable path: {path}"))?;
    if !supported_operations.iter().any(|op| op.name == operation) {
        return Err(format!(
            "Unsupported operation '{operation}' for path '{path}'"
        ));
    }

    let examples = match operation {
        "increase" => vec![OperationExample {
            request: r#"{"path":"slides[id=slide_123].style.body.font_size","op":"increase","step":1}"#,
            effect:
                "Increases body font size for the referenced slide by one point, clamped to configured bounds.",
        }],
        "decrease" => vec![OperationExample {
            request: r#"{"path":"slides[id=slide_123].style.body.font_size","op":"decrease","step":2}"#,
            effect:
                "Decreases body font size for the referenced slide by two points, clamped to configured bounds.",
        }],
        "set_alignment" => vec![OperationExample {
            request: r#"{"path":"slides[id=slide_123].style.alignment","op":"set_alignment","alignment":"left"}"#,
            effect: "Aligns text in the targeted style scope to left alignment.",
        }],
        "set_text" => vec![OperationExample {
            request: r#"{"path":"slides[id=slide_123].slots.title","op":"set_text","text":"Q3 Rollout Update"}"#,
            effect: "Replaces the target slot text with the provided string.",
        }],
        "set_layout" => vec![OperationExample {
            request: r#"{"path":"slides[id=slide_123].layout","op":"set_layout","layout":"comparison"}"#,
            effect: "Changes slide layout and triggers layout-specific required-slot checks.",
        }],
        _ => {
            return Err(format!(
                "Unsupported operation '{operation}' for path '{path}'"
            ))
        }
    };

    Ok(examples)
}
