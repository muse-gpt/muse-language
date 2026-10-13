# Maintainer scripts

Contributor / release helpers only. Do not link this file from user-facing package pages.

```powershell
cargo build --release -p muse-napi
Copy-Item target/release/muse_napi.dll target/release/muse_napi.node -Force   # Windows
# or: copy .so/.dylib → muse_napi.node on unix
cargo test --workspace
pnpm typecheck
pnpm placeholder
pnpm publish:dry
```

CLI entry is `@muse-gpt/muse` (TypeScript) composing **orthogonal** `muse-napi` primitives — not a Rust `muse.exe`.

CI publish: `scripts/ci/publish-npm.mjs`. Product docs live in the root README and package readmes.
