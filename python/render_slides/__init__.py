"""Public Python API for render-slides."""

from ._core import (
    copy_source_to_sink,
    describe_layouts,
    describe_tweaks,
    explain_operation,
    get_examples,
    get_initial_instructions,
    get_tweak_instructions,
    list_operations,
    list_paths,
    register_sink_handler,
    register_source_handler,
    render_pngs,
    render_pptx,
    validate,
)

__all__ = [
    "validate",
    "describe_layouts",
    "describe_tweaks",
    "get_initial_instructions",
    "get_tweak_instructions",
    "list_paths",
    "list_operations",
    "explain_operation",
    "get_examples",
    "copy_source_to_sink",
    "register_source_handler",
    "register_sink_handler",
    "render_pngs",
    "render_pptx",
]
