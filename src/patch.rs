use serde::Deserialize;
use serde_json::{Map, Value};

#[derive(Deserialize)]
pub(crate) struct TweakRequest {
    pub path: Option<String>,
    pub operation: String,
    #[serde(flatten)]
    pub params: Map<String, Value>,
}

pub(crate) fn apply_tweaks(ir_json: &str, tweaks_json: &str) -> Result<String, String> {
    let mut root: Value = serde_json::from_str(ir_json)
        .map_err(|e| format!("Invalid IR JSON: {e}"))?;
    
    let tweaks: Vec<TweakRequest> = serde_json::from_str(tweaks_json)
        .map_err(|e| format!("Invalid tweaks JSON array: {e}"))?;
        
    for tweak in tweaks {
        apply_single_tweak(&mut root, &tweak)?;
    }
    
    serde_json::to_string_pretty(&root)
        .map_err(|e| format!("Failed to serialize patched IR: {e}"))
}

fn apply_single_tweak(root: &mut Value, tweak: &TweakRequest) -> Result<(), String> {
    // Structural operations
    if tweak.operation == "add_slide" {
        let layout = tweak.params.get("layout").and_then(|v| v.as_str()).ok_or_else(|| "Missing 'layout' param for add_slide".to_string())?;
        let slides = root.get_mut("slides").and_then(|v| v.as_array_mut()).ok_or_else(|| "Missing $.slides array".to_string())?;
        
        let new_id = format!("slide_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis());
        
        let new_slide = serde_json::json!({
            "id": new_id,
            "layout": layout,
            "slots": {}
        });
        slides.push(new_slide);
        return Ok(());
    } else if tweak.operation == "remove_slide" {
        let id = tweak.params.get("id").and_then(|v| v.as_str()).ok_or_else(|| "Missing 'id' param for remove_slide".to_string())?;
        let slides = root.get_mut("slides").and_then(|v| v.as_array_mut()).ok_or_else(|| "Missing $.slides array".to_string())?;
        slides.retain(|s| s.get("id").and_then(|v| v.as_str()) != Some(id));
        return Ok(());
    } else if tweak.operation == "reorder_slide" {
        let id = tweak.params.get("id").and_then(|v| v.as_str()).ok_or_else(|| "Missing 'id' param for reorder_slide".to_string())?;
        let to_index = tweak.params.get("to_index").and_then(|v| v.as_i64()).ok_or_else(|| "Missing 'to_index' param".to_string())? as usize;
        let slides = root.get_mut("slides").and_then(|v| v.as_array_mut()).ok_or_else(|| "Missing $.slides array".to_string())?;
        
        let current_index = slides.iter().position(|s| s.get("id").and_then(|v| v.as_str()) == Some(id))
            .ok_or_else(|| format!("Slide '{id}' not found"))?;
            
        if to_index >= slides.len() {
            return Err(format!("to_index {to_index} out of bounds"));
        }
        
        let slide = slides.remove(current_index);
        slides.insert(to_index, slide);
        return Ok(());
    }
    
    // Path-based operations
    let path = tweak.path.as_ref().ok_or_else(|| format!("Operation '{}' requires a 'path'", tweak.operation))?;
    
    // Example path: slides[id=slide_1].style.title.font_size
    let (slide_id, rest_path) = parse_slide_id_from_path(path)?;
    
    let slides = root.get_mut("slides").and_then(|v| v.as_array_mut()).ok_or_else(|| "Missing $.slides array".to_string())?;
    let slide = slides.iter_mut()
        .find(|s| s.get("id").and_then(|v| v.as_str()) == Some(slide_id.as_str()))
        .ok_or_else(|| format!("Slide '{slide_id}' not found"))?;
        
    let parts: Vec<&str> = rest_path.split('.').collect();
    if parts.is_empty() {
        return Err("Invalid path".to_string());
    }
    
    let mut current = slide;
    for (i, part) in parts.iter().enumerate() {
        if i == parts.len() - 1 {
            // Leaf node, apply operation
            match tweak.operation.as_str() {
                "set_layout" => {
                    if *part != "layout" { return Err("set_layout must target .layout".to_string()); }
                    let layout = tweak.params.get("layout").and_then(|v| v.as_str()).ok_or_else(|| "Missing 'layout'".to_string())?;
                    current[*part] = serde_json::json!(layout);
                }
                "set_alignment" => {
                    if *part != "alignment" { return Err("set_alignment must target .alignment".to_string()); }
                    let alignment = tweak.params.get("alignment").and_then(|v| v.as_str()).ok_or_else(|| "Missing 'alignment'".to_string())?;
                    current[*part] = serde_json::json!(alignment);
                }
                "set_font_size" => {
                    let size = tweak.params.get("size").and_then(|v| v.as_i64()).ok_or_else(|| "Missing 'size'".to_string())?;
                    current[*part] = serde_json::json!(size);
                }
                "increase" => {
                    let step = tweak.params.get("step").and_then(|v| v.as_i64()).unwrap_or(1);
                    let current_val = current.get(*part).and_then(|v| v.as_i64()).unwrap_or(24);
                    current[*part] = serde_json::json!(current_val + step);
                }
                "decrease" => {
                    let step = tweak.params.get("step").and_then(|v| v.as_i64()).unwrap_or(1);
                    let current_val = current.get(*part).and_then(|v| v.as_i64()).unwrap_or(24);
                    current[*part] = serde_json::json!(std::cmp::max(10, current_val - step));
                }
                "set_text" => {
                    let text = tweak.params.get("text").and_then(|v| v.as_str()).ok_or_else(|| "Missing 'text'".to_string())?;
                    current[*part] = serde_json::json!(text);
                }
                _ => return Err(format!("Unknown path operation: {}", tweak.operation))
            }
        } else {
            // Traverse or create
            if !current.is_object() {
                *current = serde_json::json!({});
            }
            let map = current.as_object_mut().unwrap();
            if !map.contains_key(*part) {
                map.insert((*part).to_string(), serde_json::json!({}));
            }
            current = current.get_mut(*part).unwrap();
        }
    }
    
    Ok(())
}

fn parse_slide_id_from_path(path: &str) -> Result<(String, String), String> {
    if !path.starts_with("slides[id=") {
        return Err("Path must start with slides[id=".to_string());
    }
    let end_idx = path.find("]").ok_or_else(|| "Invalid path format".to_string())?;
    let id = &path[10..end_idx];
    if path.len() <= end_idx + 2 || !path[end_idx..].starts_with("].") {
        return Err("Path must continue after slides[id=...]".to_string());
    }
    let rest = &path[end_idx + 2..];
    Ok((id.to_string(), rest.to_string()))
}
