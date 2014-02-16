---
name: muse-author
description: Author and revise Muse .muse scripts with intent, orchestration, timeline, music, and visual blocks. Use when creating or editing Muse language source, diagnosing syntax, or translating creative direction into explicit rules.
---

# Muse Authoring

Use `#` for comments; `//` is invalid and must remain visible as a diagnostic. Keep creative intent in `intent` or `orchestrate` blocks and executable timing in `timeline`, `tempo`, `music`, and `visual` blocks.

Validate every edit with `muse check <script.muse>`. For machine-readable output use `muse compile <script.muse> --emit json`. Preserve source spans and avoid inventing unsupported syntax.
