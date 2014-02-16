# @muse/muse

Muse audiovisual composition language — npm entry for the `muse` CLI.

```bash
npm install -g @muse/muse
muse --help
muse doctor
```

The published `muse` bin is a **@vmz/commander** facade. Subcommands forward to the native Rust `muse` binary from an optional platform package, `PATH`, or a local `target/release/muse` when developing inside this monorepo.

Rust crates live under `projects/crates/*`. Homepage is a VMZ static site (`@muse/homepage`). Agent skills: `@muse/muse-skills`.
