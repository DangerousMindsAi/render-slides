use std::io::{Read, Write};

use pyo3::exceptions::PyValueError;
use pyo3::PyResult;
use serde_json::Value;
use zip::write::SimpleFileOptions;
use zip::ZipWriter;

use crate::ilm::resolve_ilm_slides;
use crate::ilm::model::{IlmElement, ImageScaling, TextAlignment};
use crate::schema::parse_ir;

fn xml_escape(input: &str) -> String {
    input.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn detect_image_extension(bytes: &[u8]) -> &'static str {
    if bytes.starts_with(&[0x89, b'P', b'N', b'G']) {
        return "png";
    }
    if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        return "jpg";
    }
    if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        return "gif";
    }
    "png"
}

fn add_zip_file(
    zip: &mut ZipWriter<std::io::Cursor<Vec<u8>>>,
    path: &str,
    data: &str,
) -> Result<(), String> {
    zip.start_file(path, SimpleFileOptions::default())
        .map_err(|e| format!("PPTX zip start_file error for {path}: {e}"))?;
    zip.write_all(data.as_bytes())
        .map_err(|e| format!("PPTX zip write error for {path}: {e}"))
}

pub(crate) fn build_pptx_bytes(parsed: &Value) -> Result<Vec<u8>, String> {
    let specs = resolve_ilm_slides(parsed)?;

    let mut media: Vec<(String, Vec<u8>, &'static str)> = Vec::new();
    for spec in &specs {
        for elem in &spec.elements {
            if let IlmElement::Image(image) = elem {
                let ext = detect_image_extension(&image.image_data);
                media.push((format!("image{}.{}", media.len() + 1, ext), image.image_data.clone(), ext));
            }
        }
    }

    let mut zip = ZipWriter::new(std::io::Cursor::new(Vec::<u8>::new()));
    let mut slide_rel_targets = Vec::new();
    let mut media_idx = 0usize;
    for (idx, spec) in specs.iter().enumerate() {
        let slide_number = idx + 1;
        let mut shapes_xml = String::new();
        let mut shape_id = 2usize;
        
        let mut rels_xml = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?><Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">");
        let mut has_rels = false;
        
        for elem in &spec.elements {
            match elem {
                IlmElement::Text(tb) => {
                    let run_attr = if tb.bold { " b=\"1\"" } else { "" };
                    let mut paragraphs = String::new();
                    let algn_str = match tb.alignment {
                        TextAlignment::Left => "l",
                        TextAlignment::Center => "ctr",
                        TextAlignment::Right => "r",
                        TextAlignment::Justify => "just",
                    };
                    for line in tb.text.lines() {
                        paragraphs.push_str(&format!(
                            "<a:p><a:pPr algn=\"{}\"/><a:r><a:rPr lang=\"en-US\" sz=\"{}\"{}><a:latin typeface=\"Arial\"/></a:rPr><a:t>{}</a:t></a:r></a:p>",
                            algn_str,
                            tb.font_size_pt * 100,
                            run_attr,
                            xml_escape(line)
                        ));
                    }
                    if paragraphs.is_empty() {
                        paragraphs.push_str("<a:p/>");
                    }
                    shapes_xml.push_str(&format!("<p:sp><p:nvSpPr><p:cNvPr id=\"{}\" name=\"TextBox {}\"/><p:cNvSpPr txBox=\"1\"/><p:nvPr/></p:nvSpPr><p:spPr><a:xfrm><a:off x=\"{}\" y=\"{}\"/><a:ext cx=\"{}\" cy=\"{}\"/></a:xfrm><a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom><a:noFill/></p:spPr><p:txBody><a:bodyPr wrap=\"square\" lIns=\"0\" rIns=\"0\" tIns=\"0\" bIns=\"0\"/><a:lstStyle/>{}</p:txBody></p:sp>", shape_id, shape_id, tb.x, tb.y, tb.cx, tb.cy, paragraphs));
                    shape_id += 1;
                }
                IlmElement::Image(img) => {
                    let rid = format!("rId{}", shape_id);
                    rels_xml.push_str(&format!("<Relationship Id=\"{}\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/image\" Target=\"../media/{}\"/>", rid, media[media_idx].0));
                    has_rels = true;
                    
                    let size = imagesize::blob_size(&img.image_data).unwrap_or(imagesize::ImageSize { width: 100, height: 100 });
                    let img_w = size.width as f64;
                    let img_h = size.height as f64;

                    let mut out_x = img.x;
                    let mut out_y = img.y;
                    let mut out_cx = img.cx;
                    let mut out_cy = img.cy;

                    let mut src_rect = "".to_string();

                    match img.scaling {
                        ImageScaling::Stretch => {}
                        ImageScaling::Contain => {
                            let scale = (out_cx as f64 / img_w).min(out_cy as f64 / img_h);
                            let new_cx = (img_w * scale) as i64;
                            let new_cy = (img_h * scale) as i64;
                            out_x += (out_cx - new_cx) / 2;
                            out_y += (out_cy - new_cy) / 2;
                            out_cx = new_cx;
                            out_cy = new_cy;
                        }
                        ImageScaling::Cover => {
                            let scale = (out_cx as f64 / img_w).max(out_cy as f64 / img_h);
                            let scaled_w = img_w * scale;
                            let scaled_h = img_h * scale;
                            
                            let crop_x = ((scaled_w - out_cx as f64) / 2.0 / scaled_w * 100000.0) as i64;
                            let crop_y = ((scaled_h - out_cy as f64) / 2.0 / scaled_h * 100000.0) as i64;
                            
                            src_rect = format!("<a:srcRect l=\"{}\" t=\"{}\" r=\"{}\" b=\"{}\"/>", crop_x, crop_y, crop_x, crop_y);
                        }
                        ImageScaling::FitWidth => {
                            let scale = out_cx as f64 / img_w;
                            let new_cy = (img_h * scale) as i64;
                            out_y += (out_cy - new_cy) / 2;
                            out_cy = new_cy;
                        }
                        ImageScaling::FitHeight => {
                            let scale = out_cy as f64 / img_h;
                            let new_cx = (img_w * scale) as i64;
                            out_x += (out_cx - new_cx) / 2;
                            out_cx = new_cx;
                        }
                    }
                    
                    shapes_xml.push_str(&format!("<p:pic><p:nvPicPr><p:cNvPr id=\"{}\" name=\"Image\"/><p:cNvPicPr/><p:nvPr/></p:nvPicPr><p:blipFill><a:blip r:embed=\"{}\"/>{}<a:stretch><a:fillRect/></a:stretch></p:blipFill><p:spPr><a:xfrm><a:off x=\"{}\" y=\"{}\"/><a:ext cx=\"{}\" cy=\"{}\"/></a:xfrm><a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom></p:spPr></p:pic>", shape_id, rid, src_rect, out_x, out_y, out_cx, out_cy));
                    shape_id += 1;
                    media_idx += 1;
                }
            }
        }
        rels_xml.push_str("</Relationships>");

        let slide_xml = format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?><p:sld xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\" xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\" xmlns:p=\"http://schemas.openxmlformats.org/presentationml/2006/main\"><p:cSld><p:spTree><p:nvGrpSpPr><p:cNvPr id=\"1\" name=\"\"/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr><a:xfrm><a:off x=\"0\" y=\"0\"/><a:ext cx=\"0\" cy=\"0\"/><a:chOff x=\"0\" y=\"0\"/><a:chExt cx=\"0\" cy=\"0\"/></a:xfrm></p:grpSpPr>{}</p:spTree></p:cSld><p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr></p:sld>",
            shapes_xml
        );
        add_zip_file(
            &mut zip,
            &format!("ppt/slides/slide{slide_number}.xml"),
            &slide_xml,
        )?;
        if has_rels {
            add_zip_file(
                &mut zip,
                &format!("ppt/slides/_rels/slide{slide_number}.xml.rels"),
                &rels_xml,
            )?;
        }
        slide_rel_targets.push(format!("slides/slide{slide_number}.xml"));
    }

    for (name, bytes, _) in &media {
        zip.start_file(format!("ppt/media/{name}"), SimpleFileOptions::default())
            .map_err(|e| format!("PPTX zip start_file error for media {name}: {e}"))?;
        zip.write_all(bytes)
            .map_err(|e| format!("PPTX zip write error for media {name}: {e}"))?;
    }

    let mut content_types = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?><Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\"><Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/><Default Extension=\"xml\" ContentType=\"application/xml\"/><Override PartName=\"/ppt/presentation.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml\"/><Override PartName=\"/docProps/core.xml\" ContentType=\"application/vnd.openxmlformats-package.core-properties+xml\"/><Override PartName=\"/docProps/app.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.extended-properties+xml\"/>");
    for i in 1..=slide_rel_targets.len() {
        content_types.push_str(&format!("<Override PartName=\"/ppt/slides/slide{i}.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.presentationml.slide+xml\"/>"));
    }
    for (_, _, ext) in &media {
        let ct = match *ext {
            "png" => "image/png",
            "jpg" => "image/jpeg",
            "gif" => "image/gif",
            _ => "application/octet-stream",
        };
        content_types.push_str(&format!(
            "<Default Extension=\"{}\" ContentType=\"{}\"/>",
            ext, ct
        ));
    }
    content_types.push_str("</Types>");
    add_zip_file(&mut zip, "[Content_Types].xml", &content_types)?;

    add_zip_file(&mut zip, "_rels/.rels", "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?><Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\"><Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument\" Target=\"ppt/presentation.xml\"/><Relationship Id=\"rId2\" Type=\"http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties\" Target=\"docProps/core.xml\"/><Relationship Id=\"rId3\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/extended-properties\" Target=\"docProps/app.xml\"/></Relationships>")?;
    add_zip_file(&mut zip, "docProps/app.xml", "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?><Properties xmlns=\"http://schemas.openxmlformats.org/officeDocument/2006/extended-properties\" xmlns:vt=\"http://schemas.openxmlformats.org/officeDocument/2006/docPropsVTypes\"><Application>render-slides</Application></Properties>")?;
    add_zip_file(&mut zip, "docProps/core.xml", "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?><cp:coreProperties xmlns:cp=\"http://schemas.openxmlformats.org/package/2006/metadata/core-properties\" xmlns:dc=\"http://purl.org/dc/elements/1.1/\" xmlns:dcterms=\"http://purl.org/dc/terms/\" xmlns:dcmitype=\"http://purl.org/dc/dcmitype/\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\"><dc:title>render-slides deck</dc:title><dc:creator>render-slides</dc:creator></cp:coreProperties>")?;

    let mut presentation = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?><p:presentation xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\" xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\" xmlns:p=\"http://schemas.openxmlformats.org/presentationml/2006/main\"><p:sldSz cx=\"13004800\" cy=\"7315200\" type=\"wide\"/><p:notesSz cx=\"6858000\" cy=\"9144000\"/><p:sldIdLst>");
    for i in 1..=slide_rel_targets.len() {
        presentation.push_str(&format!("<p:sldId id=\"{}\" r:id=\"rId{}\"/>", 255 + i, i));
    }
    presentation.push_str("</p:sldIdLst></p:presentation>");
    add_zip_file(&mut zip, "ppt/presentation.xml", &presentation)?;

    let mut pres_rels = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?><Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">");
    for (i, target) in slide_rel_targets.iter().enumerate() {
        pres_rels.push_str(&format!("<Relationship Id=\"rId{}\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide\" Target=\"{}\"/>", i + 1, target));
    }
    pres_rels.push_str("</Relationships>");
    add_zip_file(&mut zip, "ppt/_rels/presentation.xml.rels", &pres_rels)?;

    let cursor = zip
        .finish()
        .map_err(|e| format!("PPTX zip finalize error: {e}"))?;
    Ok(cursor.into_inner())
}

pub(crate) fn render_pptx(ir_json: &str, output_target: &str) -> PyResult<()> {
    let parsed = parse_ir(ir_json).map_err(PyValueError::new_err)?;
    let bytes = build_pptx_bytes(&parsed).map_err(PyValueError::new_err)?;
    
    // Support file:// locally as output or write to standard output path
    let mut writer = if output_target.starts_with("file://") {
        let p = output_target.strip_prefix("file://").unwrap();
        Box::new(std::fs::File::create(p).map_err(|e| PyValueError::new_err(format!("Output file error: {e}")))? ) as Box<dyn Write>
    } else {
        Box::new(std::fs::File::create(output_target).map_err(|e| PyValueError::new_err(format!("Output file error: {e}")))? ) as Box<dyn Write>
    };
    
    writer
        .write_all(&bytes)
        .map_err(|e| PyValueError::new_err(format!("Write error: {e}")))?;
    writer
        .flush()
        .map_err(|e| PyValueError::new_err(format!("Flush error: {e}")))?;
    Ok(())
}
