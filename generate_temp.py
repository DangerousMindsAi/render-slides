import sys
import json
import os
from render_slides import render_pngs

fixtures = ["markdown_lists_complex", "text_length_test_complex", "markdown_tables_complex"]
for fix in fixtures:
    with open(f"fixtures/parity/{fix}.ir.json") as f:
        ir = json.load(f)
    os.makedirs(f"gallery/renders/{fix}_temp", exist_ok=True)
    render_pngs(json.dumps(ir), f"file://{os.path.abspath('gallery/renders/'+fix+'_temp')}")
