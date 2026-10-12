# Maintainer scripts

Contributor / release helpers only. Do not link this file from user-facing package pages.

```powershell
cargo build --release -p muse-tools
cargo test --workspace
pnpm typecheck
pnpm placeholder
pnpm publish:dry
```

CI publish: `scripts/ci/publish-npm.mjs`. Product docs live in the root README and package readmes.
