import json

import pytest

import render_slides


def test_validate_accepts_minimal_ir():
    assert render_slides.validate('{"slides": []}') == "ok"


def test_validate_rejects_missing_slides():
    with pytest.raises(ValueError) as exc_info:
        render_slides.validate('{"meta": {"title": "deck"}}')
    assert "$.slides" in str(exc_info.value)


def test_validate_rejects_invalid_json():
    with pytest.raises(ValueError) as exc_info:
        render_slides.validate('{"slides": [}')
    assert "Invalid JSON" in str(exc_info.value)


def test_describe_schema_contains_expected_keys():
    schema = json.loads(render_slides.describe_schema())
    assert schema["version"] == "0.1"
    assert "title_body" in schema["slide_layouts"]
    assert "left justify" in schema["qualitative_aliases"]


def test_copy_source_to_sink_roundtrip(tmp_path):
    source = tmp_path / "source.txt"
    destination = tmp_path / "destination.txt"
    source.write_text("transport-data", encoding="utf-8")

    render_slides.copy_source_to_sink(str(source), str(destination))

    assert destination.read_text(encoding="utf-8") == "transport-data"


def test_copy_source_to_sink_rejects_unknown_scheme():
    with pytest.raises(ValueError) as exc_info:
        render_slides.copy_source_to_sink("s3://bucket/a.txt", "file:///tmp/out.txt")
    assert "Unsupported URI scheme" in str(exc_info.value)


def test_render_pngs_placeholder_raises_not_implemented():
    with pytest.raises(NotImplementedError):
        render_slides.render_pngs('{"slides": []}', "file:///tmp/slides")


def test_render_pptx_placeholder_raises_not_implemented():
    with pytest.raises(NotImplementedError):
        render_slides.render_pptx('{"slides": []}', "file:///tmp/deck.pptx")
