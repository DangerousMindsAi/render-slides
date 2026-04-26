use render_slides::schema::parse_ir;
use std::fs;
fn main() {
    let s = fs::read_to_string("fixtures/parity/markdown_tables_complex.ir.json").unwrap();
    let ir = parse_ir(&s).unwrap();
    println!("{:#?}", ir);
}
