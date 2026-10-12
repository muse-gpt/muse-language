# @muse-gpt/muse-darwin-x64

Prebuilt native `muse` binary for macOS x64. Pulled automatically with `@muse-gpt/muse` on matching hosts.

## Exports

| Export | Role |
| --- | --- |
| `bin` | Absolute path to `lib/muse` |
| `resolve()` | Same path, or throw if the binary file is missing |

Prefer depending on [`@muse-gpt/muse`](https://www.npmjs.com/package/@muse-gpt/muse); that package calls `resolve()` for you.

```bash
muse doctor
```
