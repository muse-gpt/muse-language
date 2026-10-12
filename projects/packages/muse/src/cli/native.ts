import { spawnSync } from "node:child_process";
import { existsSync } from "node:fs";
import { createRequire } from "node:module";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));

type PlatformPackage = {
    bin?: string;
    resolve?: () => string;
    default?: string;
};

function platformShort(): string {
    if (process.platform === "win32") return "win32-x64";
    if (process.platform === "darwin") {
        return process.arch === "arm64" ? "darwin-arm64" : "darwin-x64";
    }
    return process.arch === "arm64" ? "linux-arm64" : "linux-x64";
}

/** Load `@muse-gpt/muse-<platform>` and ask it for the native binary path. */
function resolveFromPlatformPackage(): string | null {
    const require = createRequire(import.meta.url);
    try {
        const plat = require(`@muse-gpt/muse-${platformShort()}`) as PlatformPackage;
        if (typeof plat.resolve === "function") {
            return plat.resolve();
        }
        const bin = plat.bin ?? plat.default;
        if (typeof bin === "string" && existsSync(bin)) {
            return bin;
        }
    } catch {
        /* optional dep missing or binary absent */
    }
    return null;
}

/** Resolve the Rust `muse` binary: platform package → PATH → workspace release. */
export function resolveNativeMuse(): string | null {
    const fromPlat = resolveFromPlatformPackage();
    if (fromPlat) return fromPlat;

    const exe = process.platform === "win32" ? "muse.exe" : "muse";
    const workspaceRelease = join(here, "..", "..", "..", "..", "..", "target", "release", exe);
    if (existsSync(workspaceRelease)) return workspaceRelease;

    return exe;
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
        if (bin === "muse" || bin === "muse.exe") {
            console.error(
                "muse: native binary not found on PATH. Install `@muse-gpt/muse` (with optional platform package) or build with `cargo build --release -p muse-tools`.",
            );
            return 1;
        }
        console.error(`muse: failed to spawn ${bin}: ${r.error.message}`);
        return 1;
    }
    return r.status ?? 1;
}
