use render_slides::schema::parse_ir;
use render_slides::ilm::resolve_ilm_slides;
use std::fs;

fn main() {
    let ir = fs::read_to_string("fixtures/parity/markdown_lists_complex.ir.json").unwrap();
    let parsed = parse_ir(&ir).unwrap();
    let slides = resolve_ilm_slides(&parsed).unwrap();
    for slide in slides {
        for elem in slide.elements {
            if let render_slides::ilm::model::IlmElement::Text(run) = elem {
                println!("Text box font size: {} pt", run.font_size_pt);
            }
        }
    }
}
