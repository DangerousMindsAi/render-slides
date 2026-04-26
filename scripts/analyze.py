import numpy as np
from PIL import Image

def get_text_bounds(img):
    arr = np.array(img.convert('L'))
    dark = arr < 128
    
    # line by line Y
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
        
    # line by line X (right edge)
    lines_x = []
    for (start_y, end_y) in lines_y:
        col_proj = np.sum(dark[start_y:end_y, :], axis=0)
        # find last column with dark pixels
        last_x = 0
        for x in range(len(col_proj)-1, -1, -1):
            if col_proj[x] > 0:
                last_x = x
                break
        lines_x.append(last_x)
        
    return lines_y, lines_x

preview = Image.open("gallery/renders/alignment_test_complex_preview_001.png")
pptx = Image.open("gallery/renders/alignment_test_complex_pptx_001.png").resize(preview.size)

p_y, p_x = get_text_bounds(preview)
x_y, x_x = get_text_bounds(pptx)

print("Preview Lines:")
for i in range(len(p_y)):
    height = p_y[i][1] - p_y[i][0]
    print(f"Line {i}: Y=({p_y[i][0]}, {p_y[i][1]}) Height={height} RightX={p_x[i]}")

print("\nPPTX Lines (Resized to Preview Dimensions):")
for i in range(len(x_y)):
    height = x_y[i][1] - x_y[i][0]
    print(f"Line {i}: Y=({x_y[i][0]}, {x_y[i][1]}) Height={height} RightX={x_x[i]}")

print("\nDifferences (PPTX - Preview):")
min_lines = min(len(p_y), len(x_y))
for i in range(min_lines):
    dy_start = x_y[i][0] - p_y[i][0]
    dy_end = x_y[i][1] - p_y[i][1]
    dx = x_x[i] - p_x[i]
    print(f"Line {i}: dY_start={dy_start}px, dY_end={dy_end}px, dX_right={dx}px")

# Also measure line gaps
print("\nLine gaps (Preview vs PPTX):")
for i in range(1, min_lines):
    p_gap = p_y[i][0] - p_y[i-1][1]
    x_gap = x_y[i][0] - x_y[i-1][1]
    print(f"Gap {i-1} to {i}: Preview={p_gap}px, PPTX={x_gap}px, Diff={x_gap - p_gap}px")
