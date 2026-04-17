"""Public Python API for render-slides."""

from ._core import describe_schema, render_pngs, render_pptx, validate, validate_detailed

__all__ = ["validate", "validate_detailed", "describe_schema", "render_pngs", "render_pptx"]
