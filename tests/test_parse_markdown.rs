use pulldown_cmark::{Event, Parser};
fn main() {
    let md = "3 × 10<sup>-418</sup>";
    let parser = Parser::new(md);
    for ev in parser {
        println!("{:?}", ev);
    }
}
