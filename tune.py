import subprocess
import re
import sys

def run_tests(kerning_factor):
    subprocess.run(["git", "restore", "src/output/png.rs"], check=True)
    with open("src/output/png.rs", "r") as f:
        content = f.read()
    
    content = content.replace("* 0.00685).max", f"* {kerning_factor:.5f}).max")
    with open("src/output/png.rs", "w") as f:
        f.write(content)
        
    subprocess.run(["maturin", "develop"], capture_output=True, check=True)
    output = subprocess.run(["python3", "run_analyze_temp.py"], capture_output=True, text=True, check=True)
    return float(output.stdout.strip())

factors = [0.01050, 0.01100, 0.01150, 0.01200, 0.01250, 0.01300]
for factor in factors:
    error = run_tests(factor)
    print(f"Factor {factor:.5f}: Error={error}")

subprocess.run(["git", "restore", "src/output/png.rs"], check=True)
