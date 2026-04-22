use serde_json::Value;

use super::model::{IlmImage, IlmSlide, IlmTextRun};
use crate::html_preview::normalize_slot_text;

fn slot_text(slots: &serde_json::Map<String, Value>, name: &str) -> String {
    normalize_slot_text(slots.get(name))
}

pub(crate) fn ilm_slide_from_ir(slide: &Value) -> Option<IlmSlide> {
    let layout = slide.get("layout")?.as_str()?;
    let slots = slide.get("slots")?.as_object()?;
    let emu = |px: i64| px * 9525;

    let spec = match layout {
        "title" => IlmSlide {
            text_runs: vec![
                IlmTextRun {
                    x: emu(96),
                    y: emu(180),
                    cx: emu(1174),
                    cy: emu(180),
                    text: slot_text(slots, "title"),
                    font_size_pt: 44,
                    bold: true,
                },
                IlmTextRun {
                    x: emu(96),
                    y: emu(390),
                    cx: emu(1174),
                    cy: emu(140),
                    text: slot_text(slots, "subtitle"),
                    font_size_pt: 28,
                    bold: false,
                },
            ],
            image: None,
        },
        "title_body" => IlmSlide {
            text_runs: vec![
                IlmTextRun {
                    x: emu(96),
                    y: emu(72),
                    cx: emu(1174),
                    cy: emu(120),
                    text: slot_text(slots, "title"),
                    font_size_pt: 40,
                    bold: true,
                },
                IlmTextRun {
                    x: emu(96),
                    y: emu(220),
                    cx: emu(1174),
                    cy: emu(430),
                    text: slot_text(slots, "body"),
                    font_size_pt: 24,
                    bold: false,
                },
            ],
            image: None,
        },
        "two_column" | "comparison" => IlmSlide {
            text_runs: vec![
                IlmTextRun {
                    x: emu(96),
                    y: emu(48),
                    cx: emu(1174),
                    cy: emu(110),
                    text: slot_text(slots, "title"),
                    font_size_pt: 36,
                    bold: true,
                },
                IlmTextRun {
                    x: emu(96),
                    y: emu(190),
                    cx: emu(560),
                    cy: emu(520),
                    text: slot_text(slots, "left"),
                    font_size_pt: 22,
                    bold: false,
                },
                IlmTextRun {
                    x: emu(710),
                    y: emu(190),
                    cx: emu(560),
                    cy: emu(520),
                    text: slot_text(slots, "right"),
                    font_size_pt: 22,
                    bold: false,
                },
            ],
            image: None,
        },
        "section" => IlmSlide {
            text_runs: vec![
                IlmTextRun {
                    x: emu(96),
                    y: emu(240),
                    cx: emu(1174),
                    cy: emu(170),
                    text: slot_text(slots, "title"),
                    font_size_pt: 46,
                    bold: true,
                },
                IlmTextRun {
                    x: emu(96),
                    y: emu(430),
                    cx: emu(1174),
                    cy: emu(120),
                    text: slot_text(slots, "subtitle"),
                    font_size_pt: 24,
                    bold: false,
                },
            ],
            image: None,
        },
        "image_focus" => IlmSlide {
            text_runs: vec![
                IlmTextRun {
                    x: emu(72),
                    y: emu(48),
                    cx: emu(1220),
                    cy: emu(90),
                    text: slot_text(slots, "title"),
                    font_size_pt: 32,
                    bold: true,
                },
                IlmTextRun {
                    x: emu(72),
                    y: emu(650),
                    cx: emu(1220),
                    cy: emu(80),
                    text: slot_text(slots, "caption"),
                    font_size_pt: 20,
                    bold: false,
                },
            ],
            image: slots
                .get("image")
                .and_then(Value::as_str)
                .map(|uri| IlmImage {
                    x: emu(170),
                    y: emu(150),
                    cx: emu(1026),
                    cy: emu(470),
                    uri: uri.to_string(),
                }),
        },
        "quote" => IlmSlide {
            text_runs: vec![
                IlmTextRun {
                    x: emu(120),
                    y: emu(180),
                    cx: emu(1120),
                    cy: emu(320),
                    text: slot_text(slots, "quote"),
                    font_size_pt: 34,
                    bold: false,
                },
                IlmTextRun {
                    x: emu(120),
                    y: emu(540),
                    cx: emu(1120),
                    cy: emu(90),
                    text: format!("— {}", slot_text(slots, "attribution")),
                    font_size_pt: 22,
                    bold: true,
                },
            ],
            image: None,
        },
        _ => return None,
    };
    Some(spec)
}

pub(crate) fn resolve_ilm_slides(parsed: &Value) -> Result<Vec<IlmSlide>, String> {
    let slides = parsed
        .get("slides")
        .and_then(Value::as_array)
        .ok_or_else(|| "ValidationError: expected $.slides to be an array.".to_string())?;
    let ilm: Vec<IlmSlide> = slides.iter().filter_map(ilm_slide_from_ir).collect();
    if ilm.len() != slides.len() {
        return Err(
            "RenderError: failed to resolve one or more slide layouts for ILM emission."
                .to_string(),
        );
    }
    Ok(ilm)
}
