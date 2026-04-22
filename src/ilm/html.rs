use serde_json::Value;

use super::model::IlmSlide;
use crate::html_preview::html_escape;
use crate::theme::render_theme_style_block;

pub(crate) fn build_single_slide_html_from_ilm(
    slide: &IlmSlide,
    theme: Option<&serde_json::Map<String, Value>>,
) -> String {
    let to_px = |emu: i64| emu / 9525;
    let mut html = String::new();
    html.push_str("<!doctype html>\n<html>\n  <head>\n");
    html.push_str(&render_theme_style_block(theme));
    html.push_str("    <style>\n      html, body { width: 1366px; height: 768px; }\n");
    html.push_str(
        "      body { margin: 0; padding: 0; overflow: hidden; position: relative; box-sizing: border-box; }\n",
    );
    html.push_str("      .ilm-text { position: absolute; white-space: pre-wrap; }\n");
    html.push_str("      .ilm-image { position: absolute; object-fit: cover; }\n");
    html.push_str("    </style>\n  </head>\n  <body>\n");
    if let Some(image) = &slide.image {
        html.push_str(&format!(
            "    <img class=\"ilm-image\" src=\"{}\" style=\"left:{}px;top:{}px;width:{}px;height:{}px;\"/>\n",
            html_escape(&image.uri),
            to_px(image.x),
            to_px(image.y),
            to_px(image.cx),
            to_px(image.cy)
        ));
    }
    for run in &slide.text_runs {
        html.push_str(&format!(
            "    <div class=\"ilm-text\" style=\"left:{}px;top:{}px;width:{}px;height:{}px;font-size:{}pt;font-weight:{};\">{}</div>\n",
            to_px(run.x),
            to_px(run.y),
            to_px(run.cx),
            to_px(run.cy),
            run.font_size_pt,
            if run.bold { "700" } else { "400" },
            html_escape(&run.text).replace('\n', "<br/>")
        ));
    }
    html.push_str("  </body>\n</html>\n");
    html
}
