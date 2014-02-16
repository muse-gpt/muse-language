---
name: muse-mcp
description: Operate Muse through its rmcp-based stdio service and its script, asset, and skill tools. Use when an agent needs structured Muse inspection, safe project edits, asset metadata, or skill suggestions.
---

# Muse MCP

Start the service with `muse mcp serve --project <project>`. Use the namespaced tools `muse.read_script`, `muse.edit_block`, `muse.suggest_skills`, and `muse.get_asset_info`.

Treat tool errors as authoritative. Project-root boundaries are enforced; after any edit, rerun Muse validation and preserve diagnostics rather than applying text substitutions that can change unrelated blocks.
