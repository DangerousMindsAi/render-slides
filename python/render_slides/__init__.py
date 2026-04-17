"""Public Python API for render-slides."""

from ._core import describe_schema, render_pngs, render_pptx, validate

__all__ = ["validate", "describe_schema", "render_pngs", "render_pptx"]
