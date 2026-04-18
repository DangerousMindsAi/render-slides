"""Public Python API for render-slides."""

from ._core import (
    copy_source_to_sink,
    describe_schema,
    explain_operation,
    get_examples,
    list_operations,
    list_paths,
    render_pngs,
    render_pptx,
    validate,
)

__all__ = [
    "validate",
    "describe_schema",
    "list_paths",
    "list_operations",
    "explain_operation",
    "get_examples",
    "copy_source_to_sink",
    "render_pngs",
    "render_pptx",
]
