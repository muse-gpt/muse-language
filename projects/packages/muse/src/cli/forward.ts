import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";

import type { ParsedOptions } from "@vmz/commander";

import { loadNative, type Diagnostic } from "../native.js";

function readString(options: ParsedOptions, key: string): string | undefined {
    const value = options[key];
    return typeof value === "string" && value.length > 0 ? value : undefined;
}

function readFlag(options: ParsedOptions, key: string): boolean {
    return options[key] === true;
}

/** Require the first positional; print a localized-style error key message. */
export function requirePositional(options: ParsedOptions, label: string): string | null {
    const value = options._[0];
    if (typeof value === "string" && value.length > 0) return value;
    console.error(`muse: missing ${label}`);
    return null;
}

function printDiagnostics(diagnostics: Diagnostic[]): void {
    for (const d of diagnostics) {
        const line = d.span?.line ?? "?";
        const col = d.span?.column ?? "?";
        console.error(`${d.severity}: ${d.message} (${line}:${col})`);
    }
}

function unavailable(feature: string): number {
    console.error(
        `muse: \`${feature}\` is not wired to muse-napi primitives yet (compose in TS when the primitive exists).`,
    );
    return 1;
}

export function forwardCheck(options: ParsedOptions): number {
    const script = requirePositional(options, "script");
    if (!script) return 2;
    try {
        const native = loadNative();
        const source = readFileSync(resolve(script), "utf8");
        const result = native.compileSource(source);
        printDiagnostics(result.diagnostics);
        if (!result.ok) return 1;
        console.log(`ok ${script}`);
        return 0;
    } catch (err) {
        console.error(`muse: ${err instanceof Error ? err.message : err}`);
        return 1;
    }
}

export function forwardCompile(options: ParsedOptions): number {
    const script = requirePositional(options, "script");
    if (!script) return 2;
    try {
        const native = loadNative();
        const emit = readString(options, "emit") ?? "json";
        if (emit === "schema") {
            console.log(JSON.stringify(native.compilationSchemaJson(), null, 2));
            return 0;
        }
        const source = readFileSync(resolve(script), "utf8");
        const result = native.compileSource(source);
        if (!result.ok) {
            printDiagnostics(result.diagnostics);
            return 1;
        }
        console.log(JSON.stringify(result.plan, null, 2));
        return 0;
    } catch (err) {
        console.error(`muse: ${err instanceof Error ? err.message : err}`);
        return 1;
    }
}

type PlanShape = {
    duration_seconds?: number;
    settings?: { width?: number; height?: number; frame_rate?: number; sample_rate?: number };
};

export function forwardPreview(options: ParsedOptions): number {
    const script = requirePositional(options, "script");
    if (!script) return 2;
    try {
        const native = loadNative();
        const scriptPath = resolve(script);
        const source = readFileSync(scriptPath, "utf8");
        const compiled = native.compileSource(source);
        if (!compiled.ok || !compiled.plan) {
            printDiagnostics(compiled.diagnostics);
            return 1;
        }
        const plan = compiled.plan as PlanShape;
        const scale = 0.1;
        const width = Math.max(16, Math.floor((plan.settings?.width ?? 1920) * scale));
        const height = Math.max(16, Math.floor((plan.settings?.height ?? 1080) * scale));
        const fps = Math.max(1, plan.settings?.frame_rate ?? 24);
        const duration = Math.max(0, plan.duration_seconds ?? 0);
        const fullFrames = Math.max(1, Math.ceil(duration * fps));
        const maxFrames = 240;
        const frames = Math.min(fullFrames, maxFrames);
        const outDir = join(dirname(scriptPath), "preview");
        mkdirSync(outDir, { recursive: true });
        const assetRoot = dirname(scriptPath);
        for (let index = 0; index < frames; index++) {
            const t = duration > 0 ? Math.min(1, index / fps / duration) : 0;
            const png = native.composeFramePng(compiled.plan, width, height, t, assetRoot);
            writeFileSync(join(outDir, `frame-${String(index).padStart(4, "0")}.png`), png);
        }
        const audioSeconds = Math.min(duration, 10, frames / fps);
        const audioSource = native.mixPlanAudioWav(
            compiled.plan,
            join(outDir, "preview.wav"),
            audioSeconds,
            assetRoot,
        );
        const manifest = {
            frames,
            width,
            height,
            scale,
            plan_duration_seconds: duration,
            rendered_duration_seconds: frames / fps,
            audio_seconds: audioSeconds,
            audio_file: "preview.wav",
            audio_source: audioSource,
        };
        writeFileSync(join(outDir, "manifest.json"), JSON.stringify(manifest, null, 2));
        console.log(`preview written to ${outDir} (${frames} frames)`);
        return 0;
    } catch (err) {
        console.error(`muse: ${err instanceof Error ? err.message : err}`);
        return 1;
    }
}

export function forwardInit(options: ParsedOptions): number {
    const pathArg = options._[0];
    const path =
        typeof pathArg === "string" && pathArg.length > 0 ? resolve(pathArg) : resolve(".");
    try {
        loadNative().initProject(path);
        console.log(`initialized ${path}`);
        return 0;
    } catch (err) {
        console.error(`muse: ${err instanceof Error ? err.message : err}`);
        return 1;
    }
}

export function forwardRender(_options: ParsedOptions): number {
    return unavailable("render");
}

export function forwardExport(_options: ParsedOptions): number {
    return unavailable("export");
}

export function forwardAssetImport(_options: ParsedOptions): number {
    return unavailable("asset import");
}

export function forwardAssetInfo(_options: ParsedOptions): number {
    return unavailable("asset info");
}

export function forwardScoreInfo(_options: ParsedOptions): number {
    return unavailable("score info");
}

export function forwardSkillList(): number {
    return unavailable("skill list");
}

export function forwardSkillValidate(): number {
    return unavailable("skill validate");
}

export function forwardMcpServe(): number {
    return unavailable("mcp serve");
}

export { readFlag, readString };
