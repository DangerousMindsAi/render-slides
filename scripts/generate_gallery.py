import argparse
import base64
import json
import os
import shutil
import subprocess
from pathlib import Path
from PIL import Image, ImageDraw

# Ensure we're in the project root
PROJECT_ROOT = Path(__file__).parent.parent.absolute()
os.chdir(PROJECT_ROOT)

def generate_test_image(text, width=200, height=100, color="red"):
    img = Image.new("RGB", (width, height), "white")
    draw = ImageDraw.Draw(img)
    draw.rectangle([0, 0, width-1, height-1], outline=color, width=5)
    draw.line([0, 0, width, height], fill=color, width=2)
    draw.line([0, height, width, 0], fill=color, width=2)
    
    tmp_path = Path(f"/tmp/{text}.png")
    img.save(tmp_path, format="PNG")
    b64 = base64.b64encode(tmp_path.read_bytes()).decode("ascii")
    tmp_path.unlink()
    return {"image_data": b64}

def generate_fixtures():
    # 1. Image Test
    layout_yaml = """layout: image_test
editable_paths: []
elements:
  stretch_img:
    x: 5%
    y: 5%
    width: 40%
    height: 40%
    image_scaling: "stretch"
  contain_img:
    x: 55%
    y: 5%
    width: 40%
    height: 40%
    image_scaling: "contain"
  cover_img:
    x: 5%
    y: 55%
    width: 40%
    height: 40%
    image_scaling: "cover"
  fit_width_img:
    x: 55%
    y: 55%
    width: 40%
    height: 40%
    image_scaling: "fit_width"
"""
    with open("templates/layouts/image_test.yaml", "w") as f:
        f.write(layout_yaml)

    wide_img = generate_test_image("wide", 300, 100, "blue")
    ir_image_test = {
        "slides": [
            {
                "layout": "image_test",
                "slots": {
                    "stretch_img": wide_img,
                    "contain_img": wide_img,
                    "cover_img": wide_img,
                    "fit_width_img": wide_img
                }
            }
        ]
    }
    with open("fixtures/parity/image_test_complex.ir.json", "w") as f:
        json.dump(ir_image_test, f, indent=2)

    # 2. Split Test
    ir_two_col = {
        "slides": [
            {
                "layout": "two_column",
                "params": {"split": 0.3},
                "slots": {
                    "title": "Uneven Split Test (30% / 70%)",
                    "left": "This is a much narrower left column because the split is 0.3. " * 5,
                    "right": "This is the right column, which should be significantly wider. " * 10
                }
            }
        ]
    }
    with open("fixtures/parity/two_column_split_complex.ir.json", "w") as f:
        json.dump(ir_two_col, f, indent=2)

    # 3. Alignment Test
    ir_align = {"slides": []}
    for align in ["left", "center", "right", "justify"]:
        ir_align["slides"].append({
            "layout": "title_body",
            "style": {"alignment": align},
            "slots": {
                "title": f"Alignment: {align}",
                "body": f"This paragraph is testing the {align} alignment feature. We are repeating this text over and over to ensure that there are multiple lines of text that will be wrapped by the Pango shaping engine and the OpenXML PowerPoint rendering engine. The visual effect should be symmetric between the two formats."
            }
        })
    with open("fixtures/parity/alignment_test_complex.ir.json", "w") as f:
        json.dump(ir_align, f, indent=2)

    # 4. Text Length Test
    ir_length = {
        "slides": [
            {
                "layout": "two_column",
                "slots": {
                    "title": "Text Length Stress Test",
                    "left": "Short text.",
                    "right": "Extremely long text. " * 150
                }
            }
        ]
    }
    with open("fixtures/parity/text_length_test_complex.ir.json", "w") as f:
        json.dump(ir_length, f, indent=2)

def generate_gallery(out_dir: Path):
    import render_slides
    
    out_dir.mkdir(parents=True, exist_ok=True)
    renders_dir = out_dir / "renders"
    renders_dir.mkdir(exist_ok=True)
    
    fixtures_dir = Path("fixtures/parity")
    markdown_content = ["# Layout Renderings\n\nCompare the generated PNG preview against the LibreOffice PPTX rendering.\n"]

    for ir_path in sorted(fixtures_dir.glob("*.ir.json")):
        stem = ir_path.name.replace(".ir.json", "")
        with open(ir_path) as fh:
            ir_json = fh.read()
        
        pptx_path = renders_dir / f"{stem}.pptx"
        render_slides.render_pptx(ir_json, str(pptx_path))
        
        png_tmp_dir = renders_dir / f"{stem}_png_tmp"
        png_tmp_dir.mkdir(exist_ok=True)
        render_slides.render_pngs(ir_json, str(png_tmp_dir))
        
        pptx_pdf_dir = renders_dir / f"{stem}_pptx_tmp"
        pptx_pdf_dir.mkdir(exist_ok=True)
        subprocess.run(["soffice", "--headless", "--convert-to", "pdf", "--outdir", str(pptx_pdf_dir), str(pptx_path)], capture_output=True)
        
        pdf_path = pptx_pdf_dir / f"{stem}.pdf"
        if pdf_path.exists():
            subprocess.run(["pdftocairo", "-png", str(pdf_path), str(pptx_pdf_dir / "slide")], capture_output=True)
        
        markdown_content.append(f"## {stem}")
        markdown_content.append(f"[Download PPTX](renders/{pptx_path.name})")
        markdown_content.append("<table><tr><th>PNG Preview</th><th>PPTX Rendering</th></tr>")
        
        png_previews = sorted(png_tmp_dir.glob("slide-*.png"))
        
        for i, png_preview in enumerate(png_previews):
            slide_idx = i + 1
            final_preview = renders_dir / f"{stem}_preview_{slide_idx:03d}.png"
            shutil.copy(png_preview, final_preview)
            
            final_pptx_png = renders_dir / f"{stem}_pptx_{slide_idx:03d}.png"
            
            # pdftocairo numbers them as -1, -2 if multiple, or just .png if single page?
            # actually pdftocairo -png with prefix 'slide' always creates slide-1.png, slide-2.png
            source_pptx_png = pptx_pdf_dir / f"slide-{slide_idx}.png"
            # Fallback if there was only 1 page and pdftocairo didn't add a number (though usually it does for -png)
            if not source_pptx_png.exists() and slide_idx == 1:
                source_pptx_png = pptx_pdf_dir / "slide.png"
                
            if source_pptx_png.exists():
                shutil.copy(source_pptx_png, final_pptx_png)
            
            markdown_content.append(f"<tr><td><img src='renders/{final_preview.name}' width='400' /><br/><em>Slide {slide_idx}</em></td>")
            if final_pptx_png.exists():
                markdown_content.append(f"<td><img src='renders/{final_pptx_png.name}' width='400' /><br/><em>Slide {slide_idx}</em></td></tr>")
            else:
                markdown_content.append("<td>Failed to render PPTX</td></tr>")
        
        markdown_content.append("</table>\n")

    gallery_md = out_dir / "preview_gallery.md"
    with open(gallery_md, "w") as fh:
        fh.write("\n".join(markdown_content))
    print(f"Gallery written to {gallery_md.absolute()}")

if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--outdir", default="gallery", help="Output directory for gallery markdown and images")
    args = parser.parse_args()
    
    print("Generating fixtures...")
    generate_fixtures()
    

    
    print("Updating parity fixtures...")
    subprocess.run(["python3", "scripts/parity_harness.py", "--update"], check=True)
    
    print("Generating gallery...")
    generate_gallery(Path(args.outdir))
    print("Done!")
