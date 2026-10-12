import { defineConfig } from "@doki-land/nifty";

/** Placeholder / Trusted Publisher package set for Muse 0.0.0 name claim. */
export default defineConfig({
    publish: {
        packages: [
            "@muse/muse-win32-x64",
            "@muse/muse-darwin-arm64",
            "@muse/muse-darwin-x64",
            "@muse/muse-linux-x64",
            "@muse/muse-linux-arm64",
            "@muse/muse-unknown-wasm32",
            "@muse/muse-skills",
            "@muse/muse",
        ],
    },
});
