# Muse

Muse is a pure-Rust audiovisual composition language and command-line tool. Scripts describe human intent, orchestration rules, musical time, layers, and assets; the compiler validates them and produces a deterministic render plan.

Muse uses three distinct authoring layers:

1. `intent` records creative context such as emotion labels and references.
2. `orchestrate` translates intent into cross-dimensional mappings, hard constraints, soft preferences, style references, and section-level transition strategy.
3. `timeline`, `music`, and `visual` declare executable content. Render engines consume the compiled plan but do not influence Muse syntax or semantics.

`directing` remains accepted as a compatibility alias for earlier scripts, but new scripts must use `orchestrate`.

## Build

```powershell
cargo build --release
.\target\release\muse.exe init demo
cd demo
..\target\release\muse.exe check main.muse
..\target\release\muse.exe preview main.muse
```

`preview` produces PNG frames, a WAV guide track, and a manifest. Asset metadata is YAML and suitable for Git. The initial native media path supports PNG/JPEG/WebP and WAV/FLAC/MP3. The command surface includes an AV1 MP4 render target, but final AV1 MP4 muxing remains gated until its pure-Rust encoder integration is enabled; it fails explicitly rather than claiming a successful MP4 export.

Local skills live under `skills/<skill-name>/skill.yaml`. Run `muse mcp serve` to expose the four local MCP tools over stdio.
