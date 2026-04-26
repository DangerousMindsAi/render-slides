use pulldown_cmark::{Parser, Event, Tag};

fn main() {
    let text = "Here is a standard markdown table testing various alignments and column widths:\n\n| Default Align | Left Align | Center Align | Right Align |\n| ------------- | :--------- | :----------: | ----------: |\n| Cell 1,1 | Cell 1,2 | Cell 1,3 | Cell 1,4 |\n| A longer text cell | Short | **Bold** text | *Italic* text |\n| Row 3 | Data | Data | Data |\n\nWe also need to make sure that text occurring after the table renders correctly.";
    let parser = Parser::new(text);
    for event in parser {
        println!("{:?}", event);
    }
}
