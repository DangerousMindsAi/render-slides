use serde_json::json;

use crate::html_preview::render_preview_html;
use crate::ilm::html::build_single_slide_html_from_ilm;
use crate::ilm::layout_map::ilm_slide_from_ir;

#[test]
fn render_preview_html_renders_template_slot_values() {
    let ir_json = r#"{
        "slides": [{
            "layout": "title_body",
            "slots": {
                "title": "Roadmap",
                "body": "Phase 1"
            }
        }]
    }"#;

    let html = render_preview_html(ir_json).expect("html preview should render");
    assert!(html.contains("layout-title-body"));
    assert!(html.contains(">Roadmap<"));
    assert!(html.contains(">Phase 1<"));
}

#[test]
fn render_preview_html_escapes_slot_html() {
    let ir_json = r#"{
        "slides": [{
            "layout": "title_body",
            "slots": {
                "title": "<script>alert(1)</script>",
                "body": "safe"
            }
        }]
    }"#;

    let html = render_preview_html(ir_json).expect("html preview should render");
    assert!(html.contains("&lt;script&gt;alert(1)&lt;/script&gt;"));
    assert!(!html.contains("<script>alert(1)</script>"));
}

#[test]
fn ilm_single_slide_html_resets_body_padding_for_geometry_parity() {
    let slide = ilm_slide_from_ir(&json!({
        "layout": "title",
        "slots": { "title": "Quarterly Update", "subtitle": "FY26" }
    }))
    .expect("ilm slide");

    let html = build_single_slide_html_from_ilm(&slide, None);
    assert!(html.contains("body { margin: 0; padding: 0;"));
}
