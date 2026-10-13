# @muse-gpt/muse

TypeScript `muse` CLI and Node helpers. Native work is done by **orthogonal `muse-napi` primitives** (loaded from optional platform packages). Do not import platform packages directly.

## Exports

| Path | Role |
| --- | --- |
| `.` | `loadNative` / primitives + `runCli` |
| `./cli` | Command-tree builders |
| `bin/muse` | End-user CLI (composes primitives in TS) |

## Native primitives (not CLI commands)

Examples: `parseSource`, `compileSource`, `composeFramePng`, `probeGpuStatus`, `loadProjectManifest`, `findFfmpegPath`, `writePlaceholderToneWav`.

Product workflows (`check` / `preview` / …) are assembled in TypeScript from these functions.

## Commands

| Command | Purpose |
| --- | --- |
| `muse check` | `compileSource` + print diagnostics |
| `muse compile` | print plan / schema JSON |
| `muse preview` | loop `composeFramePng` + placeholder WAV |
| `muse doctor` | `probeGpuStatus` + `findFfmpegPath` |
| `muse init` / `render` / `export` / … | wired as primitives land |

## Example

```bash
muse doctor
muse check main.muse
muse compile main.muse
muse preview main.muse
```

```ts
import { loadNative } from "@muse-gpt/muse";

const native = loadNative();
const { ok, plan, diagnostics } = native.compileSource(source);
```

## Install

```bash
npm install -g @muse-gpt/muse
```

Optional dependencies pull `@muse-gpt/muse-<platform>` (`muse_napi.node`) when available.
