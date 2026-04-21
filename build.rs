use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct EditablePathSpec {
    path: String,
    operation: String,
    description: String,
    params: Vec<String>,
    bounds: String,
}

#[derive(Debug, Deserialize)]
struct TemplateFrontMatter {
    layout: String,
    editable_paths: Vec<EditablePathSpec>,
}

#[derive(Debug)]
struct LayoutEntry {
    front_matter: TemplateFrontMatter,
    template_body: String,
}

fn main() {
    if let Err(err) = generate_template_manifest() {
        panic!("Template engine build step failed: {err}");
    }
}

fn generate_template_manifest() -> Result<(), String> {
    let manifest_dir = Path::new("templates/layouts");
    println!("cargo:rerun-if-changed={}", manifest_dir.display());

    let mut layout_entries: BTreeMap<String, LayoutEntry> = BTreeMap::new();

    for entry in fs::read_dir(manifest_dir)
        .map_err(|e| format!("failed to read templates directory: {e}"))?
    {
        let entry = entry.map_err(|e| format!("failed to read template entry: {e}"))?;
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("jinja") {
            continue;
        }

        println!("cargo:rerun-if-changed={}", path.display());

        let raw = fs::read_to_string(&path)
            .map_err(|e| format!("failed to read {}: {e}", path.display()))?;
        let (front_matter, template_body) = split_front_matter(&raw, &path)?;

        let parsed: TemplateFrontMatter = serde_yaml::from_str(front_matter)
            .map_err(|e| format!("invalid YAML front matter for {}: {e}", path.display()))?;

        validate_template(&parsed, template_body, &path)?;

        if layout_entries
            .insert(
                parsed.layout.clone(),
                LayoutEntry {
                    front_matter: parsed,
                    template_body: template_body.to_string(),
                },
            )
            .is_some()
        {
            return Err(format!(
                "duplicate layout definition found in {}",
                path.display()
            ));
        }
    }

    if layout_entries.is_empty() {
        return Err("no templates found under templates/layouts".to_string());
    }

    let out_dir =
        PathBuf::from(std::env::var("OUT_DIR").map_err(|e| format!("OUT_DIR unavailable: {e}"))?);
    let output = out_dir.join("template_manifest.rs");
    fs::write(&output, emit_manifest_module(&layout_entries))
        .map_err(|e| format!("failed to write {}: {e}", output.display()))?;

    Ok(())
}

fn split_front_matter<'a>(raw: &'a str, path: &Path) -> Result<(&'a str, &'a str), String> {
    if !raw.starts_with("---\n") {
        return Err(format!(
            "missing YAML front matter delimiter at top of {}",
            path.display()
        ));
    }

    let rest = &raw[4..];
    let boundary = rest.find("\n---\n").ok_or_else(|| {
        format!(
            "missing closing YAML front matter delimiter in {}",
            path.display()
        )
    })?;

    let front_matter = &rest[..boundary];
    let body = &rest[boundary + 5..];

    Ok((front_matter, body))
}

fn validate_template(
    parsed: &TemplateFrontMatter,
    template_body: &str,
    path: &Path,
) -> Result<(), String> {
    let mut slot_markers = BTreeSet::new();
    let needle = "data-slot=\"";
    let mut cursor = template_body;
    while let Some(start_idx) = cursor.find(needle) {
        let after = &cursor[start_idx + needle.len()..];
        let end_idx = after
            .find('"')
            .ok_or_else(|| format!("unterminated data-slot marker found in {}", path.display()))?;
        let slot = &after[..end_idx];
        slot_markers.insert(slot.to_string());
        cursor = &after[end_idx + 1..];
    }

    let mut metadata_slots = BTreeSet::new();
    for entry in &parsed.editable_paths {
        if !entry.path.starts_with("slides[*].") {
            return Err(format!(
                "unsupported path prefix '{}' in {}",
                entry.path,
                path.display()
            ));
        }

        if let Some(slot) = entry.path.strip_prefix("slides[*].slots.") {
            metadata_slots.insert(slot.to_string());
        }

        if entry.operation.trim().is_empty()
            || entry.description.trim().is_empty()
            || entry.bounds.trim().is_empty()
        {
            return Err(format!(
                "missing required editable_paths fields for '{}' in {}",
                entry.path,
                path.display()
            ));
        }
    }

    if metadata_slots.is_empty() && !slot_markers.is_empty() {
        return Err(format!(
            "data-slot markers present but no slot editable paths declared in {}",
            path.display()
        ));
    }

    if !metadata_slots.is_empty() && slot_markers.is_empty() {
        return Err(format!(
            "slot editable paths declared without any data-slot markers in {}",
            path.display()
        ));
    }

    let missing_in_metadata: Vec<_> = slot_markers.difference(&metadata_slots).cloned().collect();
    if !missing_in_metadata.is_empty() {
        return Err(format!(
            "data-slot markers without metadata in {}: {}",
            path.display(),
            missing_in_metadata.join(", ")
        ));
    }

    let missing_in_template: Vec<_> = metadata_slots.difference(&slot_markers).cloned().collect();
    if !missing_in_template.is_empty() {
        return Err(format!(
            "metadata slots without data-slot markers in {}: {}",
            path.display(),
            missing_in_template.join(", ")
        ));
    }

    Ok(())
}

fn emit_manifest_module(layout_entries: &BTreeMap<String, LayoutEntry>) -> String {
    let mut editable_paths = Vec::new();
    let mut operations = Vec::new();
    let mut templates = Vec::new();

    for layout in layout_entries.values() {
        for entry in &layout.front_matter.editable_paths {
            editable_paths.push(entry.path.clone());
            operations.push((
                entry.path.clone(),
                entry.operation.clone(),
                entry.description.clone(),
                entry.params.clone(),
                entry.bounds.clone(),
            ));
        }

        templates.push((
            layout.front_matter.layout.clone(),
            layout.template_body.trim().to_string(),
        ));
    }

    editable_paths.sort();
    editable_paths.dedup();
    templates.sort_by(|left, right| left.0.cmp(&right.0));

    let mut output = String::new();
    output.push_str("// @generated by build.rs; do not edit manually.\n");
    output.push_str("pub struct TemplateOperationSpec {\n");
    output.push_str("    pub path: &'static str,\n");
    output.push_str("    pub name: &'static str,\n");
    output.push_str("    pub description: &'static str,\n");
    output.push_str("    pub params: &'static [&'static str],\n");
    output.push_str("    pub bounds: &'static str,\n");
    output.push_str("}\n\n");
    output.push_str("pub struct TemplateDefinition {\n");
    output.push_str("    pub layout: &'static str,\n");
    output.push_str("    pub body: &'static str,\n");
    output.push_str("}\n\n");

    output.push_str("pub const TEMPLATE_EDITABLE_PATHS: &[&str] = &[\n");
    for path in editable_paths {
        output.push_str(&format!("    \"{}\",\n", escape_rust_string(&path)));
    }
    output.push_str("];\n\n");

    output.push_str("pub const TEMPLATE_OPERATION_SPECS: &[TemplateOperationSpec] = &[\n");
    for (path, name, description, params, bounds) in operations {
        output.push_str("    TemplateOperationSpec {\n");
        output.push_str(&format!(
            "        path: \"{}\",\n",
            escape_rust_string(&path)
        ));
        output.push_str(&format!(
            "        name: \"{}\",\n",
            escape_rust_string(&name)
        ));
        output.push_str(&format!(
            "        description: \"{}\",\n",
            escape_rust_string(&description)
        ));

        output.push_str("        params: &[");
        for (idx, param) in params.iter().enumerate() {
            if idx > 0 {
                output.push_str(", ");
            }
            output.push_str(&format!("\"{}\"", escape_rust_string(param)));
        }
        output.push_str("],\n");

        output.push_str(&format!(
            "        bounds: \"{}\",\n",
            escape_rust_string(&bounds)
        ));
        output.push_str("    },\n");
    }
    output.push_str("];\n");
    output.push_str("\n");

    output.push_str("pub const TEMPLATE_DEFINITIONS: &[TemplateDefinition] = &[\n");
    for (layout, body) in templates {
        output.push_str("    TemplateDefinition {\n");
        output.push_str(&format!(
            "        layout: \"{}\",\n",
            escape_rust_string(&layout)
        ));
        output.push_str(&format!("        body: r#\"{}\"#,\n", body));
        output.push_str("    },\n");
    }
    output.push_str("];\n");

    output
}

fn escape_rust_string(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}
