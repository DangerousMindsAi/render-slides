import json
from render_slides import render_pngs

with open("fixtures/parity/markdown_lists_complex.ir.json") as f:
    ir = json.load(f)

# Wait, `render_pngs` doesn't return the font size. But I can print it from rust code, or just guess.
