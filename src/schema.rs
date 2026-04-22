use std::sync::LazyLock;

use jsonschema::{error::ValidationErrorKind, Draft, Validator};
use serde_json::Value;

use crate::types::SchemaSummary;

const IR_SCHEMA_JSON: &str = include_str!("../schemas/v1/ir.schema.json");

static IR_VALIDATOR: LazyLock<Result<Validator, String>> = LazyLock::new(|| {
    let schema: Value = serde_json::from_str(IR_SCHEMA_JSON)
        .map_err(|err| format!("Schema compile error: invalid JSON schema: {err}"))?;
    jsonschema::options()
        .with_draft(Draft::Draft202012)
        .build(&schema)
        .map_err(|err| format!("Schema compile error: {err}"))
});

pub(crate) fn validate_ir(parsed: &Value) -> Result<(), String> {
    let validator = IR_VALIDATOR.as_ref().map_err(ToString::to_string)?;
    let mut errors = validator.iter_errors(parsed);
    if let Some(first) = errors.next() {
        return Err(format_validation_error(first));
    }

    validate_layout_required_slots(parsed)
}

pub(crate) fn parse_ir(ir_json: &str) -> Result<Value, String> {
    let parsed: Value = serde_json::from_str(ir_json).map_err(|e| format!("Invalid JSON: {e}"))?;
    validate_ir(&parsed)?;
    Ok(parsed)
}

pub(crate) fn schema_summary() -> SchemaSummary {
    SchemaSummary {
        version: "0.1",
        slide_layouts: vec![
            "title",
            "title_body",
            "two_column",
            "section",
            "image_focus",
            "quote",
            "comparison",
        ],
        qualitative_aliases: vec!["smaller", "larger", "left justify"],
    }
}

fn validate_layout_required_slots(parsed: &Value) -> Result<(), String> {
    let Some(slides) = parsed.get("slides").and_then(Value::as_array) else {
        return Ok(());
    };

    for (index, slide) in slides.iter().enumerate() {
        let Some(layout) = slide.get("layout").and_then(Value::as_str) else {
            continue;
        };

        let (required_slots, optional_slots): (&[&str], &[&str]) = match layout {
            "title" => (&["title", "subtitle"], &[]),
            "title_body" => (&["title", "body"], &["subtitle"]),
            "two_column" => (&["title", "left", "right"], &["subtitle"]),
            "section" => (&["title", "subtitle"], &[]),
            "image_focus" => (&["title", "image", "caption"], &["subtitle"]),
            "quote" => (&["quote", "attribution"], &[]),
            "comparison" => (&["title", "left", "right"], &["subtitle"]),
            _ => continue,
        };

        let Some(slots) = slide.get("slots").and_then(Value::as_object) else {
            continue;
        };

        for required_slot in required_slots {
            if !slots.contains_key(*required_slot) {
                let mut provided_slots: Vec<&str> = slots.keys().map(String::as_str).collect();
                provided_slots.sort_unstable();
                let suggested_fix = format!("Add slots.{required_slot} as a string value.");
                return Err(format!(
                    "ValidationError: missing required slot '{required_slot}' for layout '{layout}' at $.slides[{index}].slots. expected_required={required_slots:?}; optional={optional_slots:?}; provided={provided_slots:?}; suggested_fix=\"{suggested_fix}\"."
                ));
            }
        }
    }

    Ok(())
}

fn format_validation_error(error: jsonschema::ValidationError<'_>) -> String {
    let instance_path = error.instance_path().to_string();
    let path = if instance_path.is_empty() {
        "$".to_string()
    } else {
        format!("$.{instance_path}")
    };

    let hint = match error.kind() {
        ValidationErrorKind::Required { property } => {
            format!("missing required field '{property}'")
        }
        ValidationErrorKind::Type { kind } => format!("expected type {kind:?}"),
        ValidationErrorKind::Enum { .. } => {
            "value must be one of the allowed enum values".to_string()
        }
        _ => error.to_string(),
    };

    format!("ValidationError: {hint} at {path}.")
}
