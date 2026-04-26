import json
import os
import numpy as np
from PIL import Image
from render_slides import render_pngs

fixtures = ["markdown_lists_complex", "text_length_test_complex", "markdown_tables_complex"]
total_error = 0

def get_text_bounds(img):
    arr = np.array(img.convert('L'))
    dark = arr < 128
    row_proj = np.sum(dark, axis=1)
    lines_y = []
    in_line = False
    start_y = 0
    for y, count in enumerate(row_proj):
        if count > 0:
            if not in_line:
                start_y = y
                in_line = True
        else:
            if in_line:
                lines_y.append((start_y, y))
                in_line = False
    if in_line:
        lines_y.append((start_y, len(row_proj)))
    lines_x = []
    for (start_y, end_y) in lines_y:
        col_proj = np.sum(dark[start_y:end_y, :], axis=0)
        last_x = 0
        for x in range(len(col_proj)-1, -1, -1):
            if col_proj[x] > 0:
                last_x = x
                break
        lines_x.append(last_x)
    return lines_y, lines_x

for fix in fixtures:
    with open(f"fixtures/parity/{fix}.ir.json") as f:
        ir = json.load(f)
    os.makedirs(f"gallery/renders/{fix}_temp", exist_ok=True)
    render_pngs(json.dumps(ir), f"file://{os.path.abspath('gallery/renders/'+fix+'_temp')}")
    
    preview = Image.open(f"gallery/renders/{fix}_temp/slide-001.png")
    pptx = Image.open(f"gallery/renders/{fix}_pptx_001.png").resize(preview.size)
    
    p_y, p_x = get_text_bounds(preview)
    x_y, x_x = get_text_bounds(pptx)
    
    min_lines = min(len(p_y), len(x_y))
    for i in range(min_lines):
        total_error += abs(x_x[i] - p_x[i])
        
print(total_error)
