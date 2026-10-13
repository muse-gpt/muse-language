# Muse examples

The examples are source-first. Generated media belongs in a local `dist` directory and is not committed.

Compile / check via the npm CLI (NAPI primitives):

```powershell
cargo build -p muse-napi
Copy-Item target/debug/muse_napi.dll target/debug/muse_napi.node -Force
pnpm exec muse compile examples/minimal-theme.muse
pnpm exec muse check examples/minimal-theme.muse
```

Score export / render helpers that are not yet composed from napi primitives will print a clear “not wired” message until those primitives exist.

`game-battle-arc.muse` demonstrates a deterministic five-act battle arc. `minimal-theme.muse` is the smallest complete composition.
