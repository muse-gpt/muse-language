# @muse-gpt/muse-unknown-wasm32

Placeholder Wasm delivery package. **No wasm artifact ships yet** — prefer [`@muse-gpt/muse`](https://www.npmjs.com/package/@muse-gpt/muse) and the `muse` CLI. **Not a stable public import target.**

## Example

```bash
muse doctor
```

Direct import only reports the missing artifact:

```ts
import { unavailable } from "@muse-gpt/muse-unknown-wasm32";

unavailable(); // throws until a real wasm build ships
```
