use std::collections::BTreeMap;

use serde_json::Value;

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn default_theme_tokens() -> BTreeMap<&'static str, &'static str> {
    BTreeMap::from([
        ("font-family-base", "'Inter', 'Segoe UI', sans-serif"),
        ("font-size-title", "48px"),
        ("font-size-body", "28px"),
        ("line-height-title", "1.15"),
        ("line-height-body", "1.35"),
        ("space-slide-padding", "48px"),
        ("color-bg", "#ffffff"),
        ("color-text-primary", "#111111"),
        ("color-text-muted", "#4f4f4f"),
    ])
}

fn resolve_theme_token_overrides(
    theme: Option<&serde_json::Map<String, Value>>,
) -> BTreeMap<String, String> {
    let mut tokens: BTreeMap<String, String> = default_theme_tokens()
        .into_iter()
        .map(|(key, value)| (key.to_string(), value.to_string()))
        .collect();

    let Some(theme_obj) = theme else {
        return tokens;
    };

    for (source_key, token_key) in [
        ("font_family_base", "font-family-base"),
        ("font_size_title", "font-size-title"),
        ("font_size_body", "font-size-body"),
        ("line_height_title", "line-height-title"),
        ("line_height_body", "line-height-body"),
        ("space_slide_padding", "space-slide-padding"),
        ("color_bg", "color-bg"),
        ("color_text_primary", "color-text-primary"),
        ("color_text_muted", "color-text-muted"),
    ] {
        if let Some(value) = theme_obj.get(source_key).and_then(Value::as_str) {
            tokens.insert(token_key.to_string(), value.to_string());
        }
    }

    for (path, token_key) in [
        ("typography.base_font_family", "font-family-base"),
        ("typography.title_font_size", "font-size-title"),
        ("typography.body_font_size", "font-size-body"),
        ("typography.title_line_height", "line-height-title"),
        ("typography.body_line_height", "line-height-body"),
        ("spacing.slide_padding", "space-slide-padding"),
        ("colors.background", "color-bg"),
        ("colors.text_primary", "color-text-primary"),
        ("colors.text_muted", "color-text-muted"),
    ] {
        let mut cursor = Some(Value::Object(theme_obj.clone()));
        for segment in path.split('.') {
            cursor = cursor
                .and_then(|value| value.as_object().cloned().map(Value::Object))
                .and_then(|value| value.get(segment).cloned());
        }
        if let Some(Value::String(value)) = cursor {
            tokens.insert(token_key.to_string(), value);
        }
    }

    tokens
}

pub(crate) fn render_theme_style_block(theme: Option<&serde_json::Map<String, Value>>) -> String {
    let tokens = resolve_theme_token_overrides(theme);
    let mut css = String::new();
    css.push_str("    <style>\n");
    css.push_str("      :root {\n");
    for (key, value) in tokens {
        css.push_str(&format!("        --rs-{key}: {};\n", html_escape(&value)));
    }
    css.push_str("      }\n");
    css.push_str("      body {\n");
    css.push_str("        margin: 0;\n");
    css.push_str("        padding: var(--rs-space-slide-padding);\n");
    css.push_str("        background: var(--rs-color-bg);\n");
    css.push_str("        color: var(--rs-color-text-primary);\n");
    css.push_str("        font-family: var(--rs-font-family-base);\n");
    css.push_str("      }\n");
    css.push_str("    </style>\n");
    css
}
