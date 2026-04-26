import json
with open("fixtures/parity/markdown_tables_complex.ir.json") as f:
    ir = json.load(f)
from render_slides.schema import parse_ir
# wait parse_ir is in rust... Let's just run rust to print the IR
