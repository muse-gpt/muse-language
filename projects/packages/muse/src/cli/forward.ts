import { mkdirSync, readFileSync, unlinkSync, writeFileSync } from "node:fs";
import { dirname, extname, join, resolve } from "node:path";
import { tmpdir } from "node:os";

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

function ensureParent(path: string): void {
    const parent = dirname(path);
    if (parent && parent !== ".") mkdirSync(parent, { recursive: true });
}

function inferAssetRoot(scriptPath: string): string {
    let dir = dirname(scriptPath);
    for (;;) {
        try {
            readFileSync(join(dir, "muse.von"));
            return dir;
        } catch {
            /* try muse.toml */
        }
        try {
            readFileSync(join(dir, "muse.toml"));
            return dir;
        } catch {
            /* climb */
        }
        const parent = dirname(dir);
        if (parent === dir) break;
        dir = parent;
    }
    return dirname(scriptPath);
}

function extOf(path: string): string {
    return extname(path).replace(/^\./, "").toLowerCase();
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
        const assetRoot = inferAssetRoot(scriptPath);
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

export function forwardRender(options: ParsedOptions): number {
    const script = requirePositional(options, "script");
    if (!script) return 2;
    const output = readString(options, "output");
    if (!output) {
        console.error("muse: missing --output <file>");
        return 2;
    }
    const outPath = resolve(output);
    const format = (readString(options, "format") ?? (extOf(outPath) || "mp4")).toLowerCase();
    try {
        const native = loadNative();
        const scriptPath = resolve(script);
        const source = readFileSync(scriptPath, "utf8");
        ensureParent(outPath);

        if (format === "wav" || format === "mp3") {
            const score = native.composeScore(source);
            if (format === "wav") native.writeScoreWav(score, outPath);
            else native.writeScoreMp3(score, outPath);
            console.log(`rendered ${outPath}`);
            return 0;
        }

        if (format === "mp4") {
            const compiled = native.compileSource(source);
            if (!compiled.ok || !compiled.plan) {
                printDiagnostics(compiled.diagnostics);
                return 1;
            }
            const tempWav = join(tmpdir(), `muse-render-audio-${process.pid}-${Date.now()}.wav`);
            try {
                const score = native.composeScore(source);
                native.writeScoreWav(score, tempWav);
                native.encodePlanMp4(compiled.plan, outPath, {
                    assetRoot: inferAssetRoot(scriptPath),
                    audioWav: tempWav,
                });
            } finally {
                try {
                    unlinkSync(tempWav);
                } catch {
                    /* ignore */
                }
            }
            console.log(`rendered ${outPath}`);
            return 0;
        }

        console.error(`muse: unsupported render format \`${format}\`; use wav, mp3, or mp4`);
        return 2;
    } catch (err) {
        console.error(`muse: ${err instanceof Error ? err.message : err}`);
        return 1;
    }
}

export function forwardExport(options: ParsedOptions): number {
    const script = requirePositional(options, "script");
    if (!script) return 2;
    const output = readString(options, "output");
    if (!output) {
        console.error("muse: missing --output <file>");
        return 2;
    }
    const outPath = resolve(output);
    const format = (readString(options, "format") ?? extOf(outPath)).toLowerCase();
    try {
        const native = loadNative();
        const source = readFileSync(resolve(script), "utf8");
        const score = native.composeScore(source);
        ensureParent(outPath);
        switch (format) {
            case "midi":
            case "mid":
                native.writeScoreMidi(score, outPath);
                break;
            case "musicxml":
            case "xml":
                native.writeScoreMusicxml(score, outPath);
                break;
            case "pdf":
                native.writeScorePdf(score, outPath);
                break;
            case "svg":
                native.writeScoreSvg(score, outPath);
                break;
            case "html":
            case "htm":
                native.writeScoreHtml(score, outPath);
                break;
            default:
                console.error(
                    `muse: unsupported export format \`${format || "(none)"}\`; use midi, musicxml, pdf, svg, or html`,
                );
                return 2;
        }
        console.log(`exported ${outPath}`);
        return 0;
    } catch (err) {
        console.error(`muse: ${err instanceof Error ? err.message : err}`);
        return 1;
    }
}

export function forwardAssetImport(options: ParsedOptions): number {
    const file = requirePositional(options, "file");
    if (!file) return 2;
    try {
        const native = loadNative();
        const meta = native.importProjectAsset(resolve("."), resolve(file)) as {
            guid?: string;
            hash?: string;
        };
        console.log(`imported ${meta.guid ?? "?"} (${meta.hash ?? "?"})`);
        return 0;
    } catch (err) {
        console.error(`muse: ${err instanceof Error ? err.message : err}`);
        return 1;
    }
}

export function forwardAssetInfo(options: ParsedOptions): number {
    const file = requirePositional(options, "file");
    if (!file) return 2;
    try {
        const info = loadNative().readMediaInfo(resolve(file));
        console.log(JSON.stringify(info, null, 2));
        return 0;
    } catch (err) {
        console.error(`muse: ${err instanceof Error ? err.message : err}`);
        return 1;
    }
}

export function forwardScoreInfo(options: ParsedOptions): number {
    const file = requirePositional(options, "file");
    if (!file) return 2;
    try {
        const score = loadNative().loadScoreFile(resolve(file));
        console.log(JSON.stringify(score, null, 2));
        return 0;
    } catch (err) {
        console.error(`muse: ${err instanceof Error ? err.message : err}`);
        return 1;
    }
}

export function forwardSkillList(): number {
    try {
        const skills = loadNative().listProjectSkills(resolve(".")) as Array<{
            name: string;
            version: string;
            description?: string;
        }>;
        for (const skill of skills) {
            console.log(`${skill.name} ${skill.version} - ${skill.description ?? ""}`);
        }
        return 0;
    } catch (err) {
        console.error(`muse: ${err instanceof Error ? err.message : err}`);
        return 1;
    }
}

export function forwardSkillValidate(): number {
    try {
        const skills = loadNative().listProjectSkills(resolve(".")) as Array<{
            name: string;
            version: string;
        }>;
        for (const skill of skills) {
            if (!skill.name?.trim() || !skill.version?.trim()) {
                console.error("muse: skill needs a name and version");
                return 1;
            }
            console.log(`valid: ${skill.name} ${skill.version}`);
        }
        return 0;
    } catch (err) {
        console.error(`muse: ${err instanceof Error ? err.message : err}`);
        return 1;
    }
}

export function forwardMcpServe(): number {
    try {
        loadNative().serveMcpStdio(resolve("."));
        return 0;
    } catch (err) {
        console.error(`muse: ${err instanceof Error ? err.message : err}`);
        return 1;
    }
}

export { readFlag, readString };
