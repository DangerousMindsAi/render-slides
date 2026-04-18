import json
import socket
import threading

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


def test_copy_source_to_sink_http_sink_failure_raises(tmp_path):
    source = tmp_path / "source-http.txt"
    source.write_text("transport-data", encoding="utf-8")

    listener = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    listener.bind(("127.0.0.1", 0))
    listener.listen(2)
    host, port = listener.getsockname()

    def handle_requests():
        try:
            for _ in range(2):
                conn, _ = listener.accept()
                with conn:
                    conn.recv(2048)
                    conn.sendall(
                        b"HTTP/1.1 500 Internal Server Error\r\n"
                        b"Content-Length: 0\r\n"
                        b"Connection: close\r\n\r\n"
                    )
        finally:
            listener.close()

    thread = threading.Thread(target=handle_requests, daemon=True)
    thread.start()

    with pytest.raises(ValueError) as exc_info:
        render_slides.copy_source_to_sink(
            str(source),
            f"http://{host}:{port}/upload",
        )

    thread.join(timeout=2)
    assert "Flush error" in str(exc_info.value)


def test_copy_source_to_sink_rejects_unknown_scheme():
    with pytest.raises(ValueError) as exc_info:
        render_slides.copy_source_to_sink("ftp://example.com/a.txt", "file:///tmp/out.txt")
    assert "Unsupported URI scheme" in str(exc_info.value)


def test_copy_source_to_sink_rejects_invalid_s3_uri_without_key(tmp_path):
    source = tmp_path / "source.txt"
    source.write_text("transport-data", encoding="utf-8")

    with pytest.raises(ValueError) as exc_info:
        render_slides.copy_source_to_sink(str(source), "s3://bucket-only")

    assert "Invalid URI or path" in str(exc_info.value)


def test_render_pngs_placeholder_raises_not_implemented():
    with pytest.raises(NotImplementedError):
        render_slides.render_pngs('{"slides": []}', "file:///tmp/slides")


def test_render_pptx_placeholder_raises_not_implemented():
    with pytest.raises(NotImplementedError):
        render_slides.render_pptx('{"slides": []}', "file:///tmp/deck.pptx")
