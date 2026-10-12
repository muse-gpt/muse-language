import { defineConfig } from "@doki-land/nifty";

/** Placeholder / Trusted Publisher package set for Muse 0.0.0 name claim. */
export default defineConfig({
    publish: {
        packages: [
            "@muse-gpt/muse-win32-x64",
            "@muse-gpt/muse-darwin-arm64",
            "@muse-gpt/muse-darwin-x64",
            "@muse-gpt/muse-linux-x64",
            "@muse-gpt/muse-linux-arm64",
            "@muse-gpt/muse-unknown-wasm32",
            "@muse-gpt/muse-skills",
            "@muse-gpt/muse",
        ],
    },
});
