import subprocess
import re

spacings = [0, -100, -200, 100, 200, 300, 400]
best_diff = 9999
best_spacing = None

for spacing in spacings:
    # patch png.rs
    with open('src/output/png.rs', 'r') as f:
        content = f.read()
    
    # We replace the entire block with a hardcoded letter_spacing
    new_content = re.sub(r'let letter_spacing_px = .*?\n\s*let letter_spacing = .*?;', f'let letter_spacing = {spacing};', content, flags=re.DOTALL)
    
    with open('src/output/png.rs', 'w') as f:
        f.write(new_content)
        
    subprocess.run(["maturin", "develop"], capture_output=True)
    subprocess.run(["python3", "scripts/generate_gallery.py"], capture_output=True)
    res = subprocess.run(["python3", "scripts/analyze.py", "markdown_test_complex"], capture_output=True, text=True)
    
    match = re.search(r'Line 1: .* dX_right=(-?\d+)px', res.stdout)
    if match:
        diff = int(match.group(1))
        print(f"Spacing {spacing} -> Line 1 dX_right = {diff}")
        if abs(diff) < best_diff:
            best_diff = abs(diff)
            best_spacing = spacing

print(f"Best spacing: {best_spacing} with diff {best_diff}")
