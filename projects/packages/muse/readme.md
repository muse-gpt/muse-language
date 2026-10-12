# @muse-gpt/muse

npm facade for the `muse` CLI and thin Node helpers. Matching hosts pull an optional native binary automatically — **do not import platform packages directly**.

## Exports

| Path | Role |
| --- | --- |
| `.` | Package metadata and helpers (`runCli`, `resolveNativeMuse`, …) |
| `./cli` | Programmatic command-tree builders |
| `bin/muse` | End-user CLI entry |

## Commands

| Command | Purpose |
| --- | --- |
| `muse init` | Create a Muse project with a starter script |
| `muse check` | Parse and validate a Muse source file |
| `muse compile` | Compile a Muse source and print its render plan |
| `muse preview` | Render a low-resolution PNG/WAV preview |
| `muse render` | Render a score or audiovisual output |
| `muse export` | Export a composed score to a portable format |
| `muse asset` | Import and inspect local assets |
| `muse score` | Inspect MIDI and MusicXML scores |
| `muse skill` | Inspect project-local Muse skill manifests |
| `muse mcp` | Muse Model Context Protocol server |
| `muse doctor` | Check npm CLI install and native muse binary (wrapper command) |

## Example

```bash
muse doctor
muse init score_a
cd score_a
muse check main.muse
muse preview main.muse
```

```ts
import { runCli } from "@muse-gpt/muse";

await runCli(["check", "main.muse"]);
```

Product story and syntax primer: [repository README](https://github.com/muse-gpt/muse-language#readme).  
Creator skills: [`@muse-gpt/muse-skills`](https://www.npmjs.com/package/@muse-gpt/muse-skills).
