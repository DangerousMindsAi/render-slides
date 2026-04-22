use std::io::{Cursor, Read};

use serde_json::json;
use zip::ZipArchive;

use crate::output::build_pptx_bytes;

#[test]
fn build_pptx_bytes_emits_openxml_package_entries() {
    let parsed = json!({
        "slides": [{
            "layout": "title_body",
            "slots": { "title": "Roadmap", "body": "Phase 1\nPhase 2" }
        }]
    });
    let bytes = build_pptx_bytes(&parsed).expect("pptx should build");
    let mut archive = ZipArchive::new(Cursor::new(bytes)).expect("valid zip");
    assert!(archive.by_name("[Content_Types].xml").is_ok());
    assert!(archive.by_name("ppt/presentation.xml").is_ok());

    let mut slide_xml = String::new();
    archive
        .by_name("ppt/slides/slide1.xml")
        .expect("slide present")
        .read_to_string(&mut slide_xml)
        .expect("slide xml readable");
    assert!(slide_xml.contains("Roadmap"));
    assert!(slide_xml.contains("Phase 1"));
}
