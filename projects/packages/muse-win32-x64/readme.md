# @muse-gpt/muse-win32-x64

Prebuilt native `muse` binary for Windows x64. Pulled automatically with `@muse-gpt/muse` on matching hosts.

## Exports

| Export | Role |
| --- | --- |
| `bin` | Absolute path to `lib/muse.exe` |
| `resolve()` | Same path, or throw if the binary file is missing |

Prefer depending on [`@muse-gpt/muse`](https://www.npmjs.com/package/@muse-gpt/muse); that package calls `resolve()` for you.

```bash
muse doctor
```
