# LLM Agent Guide (`render-slides`)

Welcome to the `render-slides` orchestration guide! This document is explicitly written to instruct developers and their LLM agents on how to dynamically author, preview, and iterate on presentations using the Python API.

The `render-slides` engine is built around a **deterministic, layout-first** philosophy. Instead of generating raw SVG or HTML, the LLM generates an Abstract Syntax Tree (AST) mapped strictly to predefined layouts. The rendering engine handles all visual alignment, kerning, and styling.

---

## The Two-Phase Agent Workflow

To maximize token efficiency and minimize hallucinations, the API strictly segregates layout discovery from slide manipulation. A well-designed orchestrator will execute the following loop:

### Phase 1: Content Generation

1. **Discover Layouts**: The orchestrator calls `render_slides.describe_layouts()` to fetch the available layout blueprints.
2. **Prompt the Agent**: The orchestrator sends the layout descriptions to the LLM agent and prompts it to generate the initial slide content.
3. **Agent Generates AST**: The LLM agent responds with an initial JSON AST populated with Markdown content in the required slots.

### Phase 2: Iterative Tweaking

1. **Preview Generation**: The orchestrator generates a visual preview of the slide (e.g., using `render_pngs()`) and displays it to the user or a vision model.
2. **Discover Tweaks**: The orchestrator calls `render_slides.describe_tweaks()` to fetch the available modification operations.
3. **Prompt the Agent**: The orchestrator sends the tweak menu to the agent.
4. **Agent Requests Tweaks**: The agent responds with a list of discrete operations (e.g., "increase font size", "set layout to two_column").
5. **Orchestrator Applies Tweaks**: The orchestrator patches the AST and reruns the loop until satisfied.

---

## Prompting Examples & Formats

> [!TIP]
> **Token Optimization:** When injecting schema definitions into your LLM prompts, always convert the JSON output from the `render-slides` API into **YAML**. YAML saves up to 40% on context tokens by eliminating brackets and quotes, while maintaining near-perfect comprehension by modern models.

### Example: Prompting for Phase 1 (Layout Selection)

**Python Context Preparation:**
```python
import yaml, json, render_slides

layouts = json.loads(render_slides.describe_layouts())
yaml_schema = yaml.dump(layouts, sort_keys=False)
```

**System Prompt Example:**
```text
You are an expert presentation designer. Your task is to generate an initial slide deck.
You MUST choose layouts from the following available blueprints:

<layouts>
{yaml_schema}
</layouts>

Generate a strictly valid JSON response matching this schema:
{
  "slides": [
    {
      "layout": "title_body",
      "slots": {
        "title": "My Title",
        "body": "Markdown text here"
      }
    }
  ]
}
```

### Example: Prompting for Phase 2 (Iterative Tweaking)

**Python Context Preparation:**
```python
tweaks = json.loads(render_slides.describe_tweaks())
yaml_tweaks = yaml.dump(tweaks, sort_keys=False)
```

**System Prompt Example:**
```text
You are reviewing the generated presentation. If you need to modify the slides, you may ONLY use the following tweaking operations:

<available_tweaks>
{yaml_tweaks}
</available_tweaks>

To apply a tweak, return an array of operations. For example, to make the title on slide 0 larger:
[
  {
    "path": "slides[0].style.title.font_size",
    "operation": "increase",
    "step": 1
  }
]
```

---

## Reference Guide: Tweak Categories

The `describe_tweaks()` API groups operations into three categories to help your agent reason about side-effects:

### 1. Qualitative Tweaks
Operations that conceptually shift the slide without requiring absolute values.
- `increase` / `decrease` (font sizes, margins)
- `set_alignment` (left, center, right)
- `set_layout` (change a slide's active layout blueprint)

### 2. Quantitative Tweaks
Operations that force an absolute numeric or string value.
- `set_font_size` (forces a specific integer size between 10..72)
- `set_text` (completely replaces the content in a slot)

### 3. Structural Operations
Operations that modify the array of slides.
- `add_slide` (appends a new slide)
- `remove_slide` (deletes a slide by index)
- `reorder_slide` (shifts a slide to a new index)

---

## Integration Checklist for Developers

- [ ] Ensure you handle `ValueError` exceptions raised by `render_slides.validate()` if the LLM hallucinates an unsupported slot.
- [ ] Provide the agent with clear error feedback if an operation targets an invalid path (e.g., trying to use `set_text` on a `slides[*].style` object).
- [ ] Implement a JSON Patch or dict-update utility in your orchestrator to apply the agent's tweak requests to your in-memory AST before re-rendering.
