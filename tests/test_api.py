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


def test_validate_detailed_reports_multiple_errors():
    payload = json.loads(
        render_slides.validate_detailed(
            '{"slides":[{"layout":"title_body","slots":{"title":"Only title"}}]}'
        )
    )
    assert payload["valid"] is False
    assert payload["error_count"] >= 1
    assert any("$.slides[0].slots.body" in err for err in payload["errors"])


def test_validate_rejects_unknown_layout():
    with pytest.raises(ValueError) as exc_info:
        render_slides.validate(
            '{"slides":[{"layout":"not_a_layout","slots":{"title":"x"}}]}'
        )
    assert "unsupported layout" in str(exc_info.value)


def test_render_pngs_placeholder_raises_not_implemented():
    with pytest.raises(NotImplementedError):
        render_slides.render_pngs('{"slides": []}', "file:///tmp/slides")


def test_render_pptx_placeholder_raises_not_implemented():
    with pytest.raises(NotImplementedError):
        render_slides.render_pptx('{"slides": []}', "file:///tmp/deck.pptx")
