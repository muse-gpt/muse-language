# Muse Language for VS Code

Syntax highlighting and basic language configuration for `.muse` audiovisual composition scripts.

The extension registers the same TextMate grammar and token colors used by the Muse homepage's Shiki code previews:

- `syntaxes/muse.tmLanguage.json` defines Muse scopes.
- `themes/muse-color-theme.json` defines the shared token palette.
- `.muse` files receive comments, declarations, properties, strings, numbers, and section-name highlighting.

Run `pnpm build` from this directory to bundle the command extension. The `Muse: Open Language Guide` command opens the local homepage guide during development.
