/**
 * Load orthogonal muse-napi primitives (not a CLI command surface).
 * Resolution: platform package → workspace target → error.
 */
import { createRequire } from "node:module";
import { existsSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const require = createRequire(import.meta.url);
const here = dirname(fileURLToPath(import.meta.url));

export type Diagnostic = {
    severity: "Error" | "Warning" | string;
    message: string;
    span?: { start?: number; end?: number; line?: number; column?: number };
};

export type ParseResult = {
    diagnostics: Diagnostic[];
    script: unknown | null;
    ok: boolean;
};

export type CompileResult = {
    diagnostics: Diagnostic[];
    plan: unknown | null;
    ok: boolean;
};

export type RenderSettingsInput = {
    width?: number;
    height?: number;
    frame_rate?: number;
    sample_rate?: number;
};

export type EncodeMp4Options = {
    assetRoot?: string | null;
    audioWav?: string | null;
    maxDurationSeconds?: number | null;
};

/** Orthogonal primitives exported by `muse-napi`. */
export type MuseNative = {
    parseSource(source: string): ParseResult;
    compileSource(
        source: string,
        settings?: RenderSettingsInput | null,
        assetRoot?: string | null,
    ): CompileResult;
    compilationSchemaJson(): unknown;
    loadProjectManifest(projectPath: string): unknown;
    validateProjectManifest(manifest: unknown): void;
    findProjectEntry(manifest: unknown, entryId?: string | null): unknown;
    /** Validate assets/prefabs VON definitions; returns prefab ids. */
    checkProjectPrefabs(projectPath: string): string[];
    initProject(path: string): void;
    probeGpuStatus(): {
        status: string;
        preferred: string;
        adapter_name?: string | null;
        backend?: string | null;
        device_type?: string | null;
        message: string;
    };
    findFfmpegPath(): string | null;
    composeFramePng(
        plan: unknown,
        width: number,
        height: number,
        t: number,
        assetRoot?: string | null,
    ): Buffer;
    writePlaceholderToneWav(path: string, seconds: number, sampleRate: number): void;
    mixPlanAudioWav(
        plan: unknown,
        outPath: string,
        seconds: number,
        assetRoot?: string | null,
    ): string;
    composeScore(source: string): unknown;
    writeScoreWav(score: unknown, path: string): void;
    writeScoreMp3(score: unknown, path: string): void;
    writeScoreMidi(score: unknown, path: string): void;
    writeScoreMusicxml(score: unknown, path: string): void;
    writeScorePdf(score: unknown, path: string): void;
    writeScoreSvg(score: unknown, path: string): void;
    writeScoreHtml(score: unknown, path: string): void;
    loadScoreFile(path: string): unknown;
    encodePlanMp4(plan: unknown, outPath: string, options?: EncodeMp4Options | null): void;
    hashFileSha256(path: string): unknown;
    importProjectAsset(projectPath: string, inputPath: string): unknown;
    readMediaInfo(path: string): unknown;
    listProjectSkills(projectPath: string): unknown;
    /** Blocks until the MCP client disconnects. */
    serveMcpStdio(projectPath: string): void;
};

function platformShort(): string {
    if (process.platform === "win32") return "win32-x64";
    if (process.platform === "darwin") {
        return process.arch === "arm64" ? "darwin-arm64" : "darwin-x64";
    }
    return process.arch === "arm64" ? "linux-arm64" : "linux-x64";
}

function tryLoad(path: string): MuseNative | null {
    if (!existsSync(path)) return null;
    try {
        return require(path) as MuseNative;
    } catch {
        return null;
    }
}

function resolveFromPlatformPackage(): MuseNative | null {
    try {
        const plat = require(`@muse-gpt/muse-${platformShort()}`) as {
            resolveAddon?: () => string;
            resolve?: () => string;
            addon?: string;
        };
        const path =
            (typeof plat.resolveAddon === "function" && plat.resolveAddon()) ||
            (typeof plat.addon === "string" && plat.addon) ||
            null;
        if (typeof path === "string") {
            return tryLoad(path);
        }
        // Legacy resolve() pointed at muse.exe — ignore for napi.
    } catch {
        /* optional */
    }
    return null;
}

function resolveFromWorkspace(): MuseNative | null {
    // src/ → package → packages → projects → repo root
    const root = join(here, "..", "..", "..", "..");
    const names = [
        join(root, "target", "release", "muse_napi.node"),
        join(root, "target", "debug", "muse_napi.node"),
        join(root, "target", "release", "muse_napi.dll"),
        join(root, "target", "debug", "muse_napi.dll"),
    ];
    for (const path of names) {
        const loaded = tryLoad(path);
        if (loaded) return loaded;
    }
    return null;
}

let cached: MuseNative | null | undefined;

/** Load native primitives once. Throws if unavailable. */
export function loadNative(): MuseNative {
    if (cached) return cached;
    const native = resolveFromPlatformPackage() ?? resolveFromWorkspace();
    if (!native) {
        throw new Error(
            "muse: native addon (muse-napi) not found. Build with `cargo build -p muse-napi` (then copy/rename to muse_napi.node) or install the platform package.",
        );
    }
    cached = native;
    return native;
}

export function tryLoadNative(): MuseNative | null {
    try {
        return loadNative();
    } catch {
        return null;
    }
}
