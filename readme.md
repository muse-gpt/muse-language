# Muse

[Examples](./examples) · [`@muse-gpt/muse`](https://www.npmjs.com/package/@muse-gpt/muse) · [`@muse-gpt/muse-skills`](https://www.npmjs.com/package/@muse-gpt/muse-skills)

## 💡 What is Muse?

Muse is an **audiovisual composition language**. You write intent, orchestration rules, musical time, layers, and assets in a `.muse` script; the toolchain validates the script and produces a **deterministic** render plan. Same source and inputs always yield a reviewable result.

Muse is **not** a timeline editor GUI, and it is **not** a black box that pastes prompts into a renderer. Render engines only consume the plan — they do not define Muse syntax or semantics.

Authoring has three layers:

1. `intent` — creative context (emotion, references, …)
2. `orchestrate` — cross-cutting maps, hard constraints, soft preferences, section transitions (`directing` remains a compatibility alias)
3. `timeline` / `music` / `visual` — executable content

## 🚀 Getting started

### 1. Agent / prompt (recommended)

```bash
npm install -g @muse-gpt/muse
npx @muse-gpt/muse-skills
```

Then ask your coding agent:

```text
Write a minimal .muse for this theme with intent, orchestrate, timeline, and one melodic motif, then run muse check.
```

### 2. Manual install

```bash
npm install -g @muse-gpt/muse
muse doctor
muse init score_a
cd score_a
muse check main.muse
muse preview main.muse
```

`preview` writes low-resolution PNG frames, a WAV guide track, and a manifest.

## ✨ Highlights

- **Intent → plan** — lock constraints and structure before export
- **Deterministic** — plans and previews are reviewable, not one-shot luck
- **Multi-entry projects** — a project can declare several entries; each entry can declare several outputs
- **Creator skills** — workflows for composing, shorts, explainers, publish, … — not compiler internals

## 📊 How Muse compares

| | Muse | DAW / NLE | Prompt-to-video kits |
| --- | --- | --- | --- |
| Source of truth | `.muse` script + plan | Timeline UI | Chat / prompts |
| Reproducibility | Same input → same plan | Session-dependent | Often non-deterministic |
| Agent-friendly | First-class text + skills | Weak | Strong prompts, weak contract |
| What you ship | Validated plan + exports | Project file / renders | Clip |

## 📝 Syntax primer

Comments use `#`.

```muse
namespace motif_a;

intent {
  emotion "urgent resolve" intensity 0.8 for theme
}

orchestrate for theme {
  hard_constraints: ["keep the motif recognizable"]
  soft_preferences: ["strings before brass"]
}

timeline {
  act theme : 0..8
}

tempo { at 0 bpm 120 }

music for theme {
  key "C major"
  meter 4/4
  motif "C4 E4 G4 C5"
  template "game_battle"
}
```

## 🧪 Examples

| Example | What it shows |
| --- | --- |
| [`minimal-theme.muse`](./examples/minimal-theme.muse) | Smallest complete composition |
| [`game-battle-arc.muse`](./examples/game-battle-arc.muse) | Multi-act arc + `orchestrate` |
| [`accidentals.muse`](./examples/accidentals.muse) | Accidentals in a motif |

```bash
muse check examples/minimal-theme.muse
muse export examples/minimal-theme.muse --format midi -o out.mid
```
