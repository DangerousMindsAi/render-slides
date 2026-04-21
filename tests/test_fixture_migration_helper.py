import base64
import subprocess
import sys
from pathlib import Path


def test_migration_helper_converts_binary_renderer_fixture(tmp_path):
    fixture_path = tmp_path / "title_basic.render.png"
    fixture_path.write_bytes(b"\x89PNG\r\n\x1a\n\x00\x00")

    script = Path(__file__).resolve().parents[1] / "scripts" / "migrate_renderer_fixtures_to_base64.py"
    subprocess.run(
        [sys.executable, str(script), "--fixtures-dir", str(tmp_path)],
        check=True,
        capture_output=True,
        text=True,
    )

    encoded_path = tmp_path / "title_basic.render.png.base64"
    assert encoded_path.exists()
    assert encoded_path.read_text(encoding="utf-8") == base64.b64encode(b"\x89PNG\r\n\x1a\n\x00\x00").decode("ascii") + "\n"
    assert not fixture_path.exists()
