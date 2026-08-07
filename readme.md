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

## Creator Skills

Repository skills are organized around the person and the work they are doing, not around compiler internals:

- `muse-creator`: choose a workflow from an open-ended idea.
- `muse-composer`: compose, arrange, notate, and export music.
- `muse-science-video`: build fact-checked science and knowledge explainers.
- `muse-short-video`: design vertical short-form videos and platform variants.
- `muse-filmmaker`: develop scenes, shot language, narrative arcs, and sound design.
- `muse-visual-artist`: create visual systems, motion, particles, and generative scenes.
- `muse-lesson`: turn learning objectives into teachable audiovisual lessons.
- `muse-podcast`: structure podcasts, interviews, and sound narratives.
- `muse-game-cinematic`: coordinate game states, music, and cinematics.
- `muse-publish`: validate, preview, and package deliverables for release.

Each skill has a `SKILL.md`, an audience-specific trigger description, and only the references needed for that workflow. Project-local versioned manifests remain under `skills/<skill-name>/skill.yaml`; run `muse mcp serve` to expose the four project tools over stdio.
