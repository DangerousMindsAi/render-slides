use serde_json::json;

use crate::operations::operation_specs_for;
use crate::schema::{schema_summary, validate_ir};
use crate::templating::all_editable_paths;

#[test]
fn validate_ir_accepts_minimal_valid_input() {
    let parsed = json!({ "slides": [] });
    assert!(validate_ir(&parsed).is_ok());
}

#[test]
fn validate_ir_rejects_missing_slides() {
    let parsed = json!({ "meta": { "title": "x" } });
    let err = validate_ir(&parsed).expect_err("expected missing-slides validation error");
    assert!(err.contains("ValidationError"));
    assert!(err.contains("missing required field"));
}

#[test]
fn validate_ir_rejects_non_array_slides() {
    let parsed = json!({ "slides": {} });
    let err = validate_ir(&parsed).expect_err("expected non-array slides validation error");
    assert!(err.contains("ValidationError"));
    assert!(err.contains("expected type"));
}

#[test]
fn schema_summary_contains_expected_layouts_and_aliases() {
    let summary = schema_summary();
    assert_eq!(summary.version, "0.1");
    assert!(summary.slide_layouts.contains(&"title_body"));
    assert!(summary.qualitative_aliases.contains(&"left justify"));
}

#[test]
fn operation_specs_exist_for_known_path() {
    let operations =
        operation_specs_for("slides[*].style.body.font_size").expect("operations should exist");
    assert!(operations.iter().any(|op| op.name == "increase"));
    assert!(operations.iter().any(|op| op.name == "decrease"));
}

#[test]
fn template_manifest_operations_are_available() {
    let operations = operation_specs_for("slides[*].slots.title")
        .expect("template-generated operation should be available");
    assert!(operations.iter().any(|op| op.name == "set_text"));
}

#[test]
fn quote_layout_template_operations_are_available() {
    let operations = operation_specs_for("slides[*].slots.quote")
        .expect("quote slot operation should be available");
    assert!(operations.iter().any(|op| op.name == "set_text"));
}

#[test]
fn template_manifest_paths_snapshot_is_stable() {
    assert_eq!(
        all_editable_paths(),
        vec![
            "slides[*].layout",
            "slides[*].slots.attribution",
            "slides[*].slots.body",
            "slides[*].slots.caption",
            "slides[*].slots.image",
            "slides[*].slots.left",
            "slides[*].slots.quote",
            "slides[*].slots.right",
            "slides[*].slots.subtitle",
            "slides[*].slots.title",
            "slides[*].style.alignment",
            "slides[*].style.body.font_size",
        ]
    );
}

#[test]
fn operation_specs_missing_for_unknown_path() {
    assert!(operation_specs_for("slides[*].slots.unknown").is_none());
}

#[test]
fn validate_ir_rejects_missing_required_slot_for_layout() {
    let parsed = json!({
        "slides": [{
            "layout": "title_body",
            "slots": { "title": "Missing body slot" }
        }]
    });

    let err = validate_ir(&parsed).expect_err("expected required-slot validation error");
    assert!(err.contains("missing required slot 'body'"));
    assert!(err.contains("$.slides[0].slots"));
}

#[test]
fn validate_ir_accepts_required_slots_for_layout() {
    let parsed = json!({
        "slides": [{
            "layout": "comparison",
            "slots": { "title": "Tradeoffs", "left": "Pros", "right": "Cons" }
        }]
    });

    assert!(validate_ir(&parsed).is_ok());
}
