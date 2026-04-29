use pulldown_cmark::{Event, Parser};
#[test]
fn test_print() {
    let md = "3 × 10<sup>-418</sup>";
    for ev in Parser::new(md) {
        println!("{:?}", ev);
    }
}
