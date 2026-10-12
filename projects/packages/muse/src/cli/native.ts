import { spawnSync } from "node:child_process";
import { existsSync } from "node:fs";
import { createRequire } from "node:module";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));

function platformShort(): string {
    if (process.platform === "win32") return "win32-x64";
    if (process.platform === "darwin") {
        return process.arch === "arm64" ? "darwin-arm64" : "darwin-x64";
    }
    return process.arch === "arm64" ? "linux-arm64" : "linux-x64";
}

/** Resolve the Rust `muse` binary: optional platform package → PATH → workspace release. */
export function resolveNativeMuse(): string | null {
    const require = createRequire(import.meta.url);
    const exe = process.platform === "win32" ? "muse.exe" : "muse";
    const candidates: string[] = [];

    try {
        const pkgRoot = dirname(require.resolve(`@muse-gpt/muse-${platformShort()}/package.json`));
        candidates.push(join(pkgRoot, "lib", exe));
    } catch {
        /* optional dep missing */
    }

    candidates.push(exe);
    candidates.push(join(here, "..", "..", "..", "..", "..", "target", "release", exe));

    for (const bin of candidates) {
        if (bin === exe) return bin;
        if (existsSync(bin)) return bin;
    }
    return null;
}

/** Forward argv to the native CLI. Returns process exit code. */
export function runNative(argv: string[]): number {
    const bin = resolveNativeMuse();
    if (!bin) {
        console.error(
            "muse: native binary not found. Run `cargo build --release -p muse-tools` or install a platform package.",
        );
        return 1;
    }
    const r = spawnSync(bin, argv, { stdio: "inherit", shell: false });
    if (r.error) {
        if (bin === "muse") {
            console.error(
                "muse: native binary not found on PATH. Build with `cargo build --release -p muse-tools`.",
            );
            return 1;
        }
        console.error(`muse: failed to spawn ${bin}: ${r.error.message}`);
        return 1;
    }
    return r.status ?? 1;
}
