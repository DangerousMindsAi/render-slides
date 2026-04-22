use std::collections::{BTreeMap, BTreeSet};

use crate::generated;
use crate::types::SlideTemplate;

pub(crate) fn all_editable_paths() -> Vec<&'static str> {
    let mut unique = BTreeSet::new();
    unique.extend(generated::TEMPLATE_EDITABLE_PATHS.iter().copied());
    unique.into_iter().collect()
}

pub(crate) fn supports_path(path: &str) -> bool {
    all_editable_paths().contains(&path)
}

pub(crate) fn template_registry() -> BTreeMap<&'static str, SlideTemplate> {
    generated::TEMPLATE_DEFINITIONS
        .iter()
        .map(|entry| {
            (
                entry.layout,
                SlideTemplate {
                    body: entry.body,
                    slot_names: collect_slot_names(entry.body),
                },
            )
        })
        .collect()
}

pub(crate) fn collect_slot_names(template_body: &str) -> Vec<String> {
    let mut slot_names = BTreeSet::new();
    let mut cursor = template_body;
    let needle = "data-slot=\"";

    while let Some(start_idx) = cursor.find(needle) {
        let after = &cursor[start_idx + needle.len()..];
        let Some(end_idx) = after.find('"') else {
            break;
        };
        slot_names.insert(after[..end_idx].to_string());
        cursor = &after[end_idx + 1..];
    }

    slot_names.into_iter().collect()
}
