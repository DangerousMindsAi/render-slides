"""Public Python API for render-slides."""

from ._core import copy_source_to_sink, describe_schema, render_pngs, render_pptx, validate

__all__ = ["validate", "describe_schema", "copy_source_to_sink", "render_pngs", "render_pptx"]
