import { mkdirSync, readFileSync, unlinkSync, writeFileSync } from "node:fs";
import { dirname, extname, isAbsolute, join, resolve } from "node:path";
import { tmpdir } from "node:os";

import type { ParsedOptions } from "@vmz/commander";

import {
    loadNative,
    type Diagnostic,
    type MuseNative,
    type RenderSettingsInput,
} from "../native.js";

function readString(options: ParsedOptions, key: string): string | undefined {
    const value = options[key];
    return typeof value === "string" && value.length > 0 ? value : undefined;
}

function readFlag(options: ParsedOptions, key: string): boolean {
    return options[key] === true;
}

function readNumber(options: ParsedOptions, key: string): number | undefined {
    const value = options[key];
    if (typeof value === "number" && Number.isFinite(value)) return value;
    if (typeof value === "string" && value.length > 0) {
        const n = Number(value);
        if (Number.isFinite(n)) return n;
    }
    return undefined;
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

type RenderOverride = {
    width?: number | null;
    height?: number | null;
    frame_rate?: number | null;
};

type OutputSpec = {
    id: string;
    kind: string;
    path: string;
    format?: string | null;
    render?: RenderOverride | null;
};

type ProjectEntry = {
    id: string;
    source: string;
    render?: RenderOverride | null;
    outputs?: OutputSpec[];
};

type ProjectManifest = {
    project?: { id?: string };
    render?: { width?: number; height?: number; frame_rate?: number };
    entries: ProjectEntry[];
};

type PlanShape = {
    duration_seconds?: number;
    audio_source?: string;
    audio_tracks?: unknown[];
    settings?: { width?: number; height?: number; frame_rate?: number; sample_rate?: number };
};

/** Merge project → entry → output render (later non-null fields win). */
function resolveRenderSettings(
    manifest: ProjectManifest,
    entry?: ProjectEntry | null,
    output?: OutputSpec | null,
): RenderSettingsInput {
    const pick = (
        key: keyof RenderOverride,
    ): number | undefined => {
        const fromOutput = output?.render?.[key];
        if (typeof fromOutput === "number") return fromOutput;
        const fromEntry = entry?.render?.[key];
        if (typeof fromEntry === "number") return fromEntry;
        const fromProject = manifest.render?.[key];
        return typeof fromProject === "number" ? fromProject : undefined;
    };
    return {
        width: pick("width"),
        height: pick("height"),
        frame_rate: pick("frame_rate"),
    };
}

function loadManifest(native: MuseNative, projectRoot: string): ProjectManifest {
    const manifest = native.loadProjectManifest(projectRoot) as ProjectManifest;
    native.validateProjectManifest(manifest);
    return manifest;
}

function resolveEntries(
    native: MuseNative,
    manifest: ProjectManifest,
    entryId: string | undefined,
): ProjectEntry[] {
    if (entryId) {
        return [native.findProjectEntry(manifest, entryId) as ProjectEntry];
    }
    return manifest.entries ?? [];
}

export function forwardCheck(options: ParsedOptions): number {
    const projectOpt = readString(options, "project");
    try {
        const native = loadNative();
        if (projectOpt) {
            const projectRoot = resolve(projectOpt);
            const manifest = loadManifest(native, projectRoot);
            const entries = resolveEntries(native, manifest, readString(options, "entry"));
            let failed = false;
            for (const entry of entries) {
                const scriptPath = join(projectRoot, entry.source);
                console.log(`check entry \`${entry.id}\` (${scriptPath})`);
                const source = readFileSync(scriptPath, "utf8");
                const result = native.compileSource(
                    source,
                    resolveRenderSettings(manifest, entry),
                    projectRoot,
                );
                printDiagnostics(result.diagnostics);
                if (!result.ok) {
                    failed = true;
                    continue;
                }
                for (const output of entry.outputs ?? []) {
                    if (!output.path?.trim()) {
                        console.error(
                            `muse: entry \`${entry.id}\` output \`${output.id}\` has an empty path`,
                        );
                        failed = true;
                        continue;
                    }
                    if (
                        (output.kind === "preview" ||
                            output.kind === "video" ||
                            output.kind === "audio") &&
                        isAbsolute(output.path)
                    ) {
                        console.error(
                            `muse: entry \`${entry.id}\` output \`${output.id}\` must use a project-relative path`,
                        );
                        failed = true;
                    }
                }
            }
            if (failed) return 1;
            console.log(`project ok (${projectRoot})`);
            return 0;
        }

        const script = requirePositional(options, "script");
        if (!script) return 2;
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

function runPreview(args: {
    native: MuseNative;
    scriptPath: string;
    source: string;
    settings?: RenderSettingsInput | null;
    assetRoot: string;
    outDir: string;
}): number {
    const { native, scriptPath, source, settings, assetRoot, outDir } = args;
    const compiled = native.compileSource(source, settings, assetRoot);
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
    mkdirSync(outDir, { recursive: true });
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
        script: scriptPath,
    };
    writeFileSync(join(outDir, "manifest.json"), JSON.stringify(manifest, null, 2));
    console.log(`preview written to ${outDir} (${frames} frames)`);
    return 0;
}

export function forwardPreview(options: ParsedOptions): number {
    try {
        const native = loadNative();
        const projectOpt = readString(options, "project");
        if (projectOpt) {
            const projectRoot = resolve(projectOpt);
            const manifest = loadManifest(native, projectRoot);
            const entry = resolveEntries(native, manifest, readString(options, "entry"))[0];
            if (!entry) {
                console.error("muse: project has no entries");
                return 1;
            }
            const scriptPath = join(projectRoot, entry.source);
            const previewOut =
                entry.outputs?.find((o) => o.kind === "preview")?.path ??
                join("library", entry.id, "preview");
            const previewSpec = entry.outputs?.find((o) => o.kind === "preview");
            return runPreview({
                native,
                scriptPath,
                source: readFileSync(scriptPath, "utf8"),
                settings: resolveRenderSettings(manifest, entry, previewSpec),
                assetRoot: projectRoot,
                outDir: join(projectRoot, previewOut),
            });
        }

        const script = requirePositional(options, "script");
        if (!script) return 2;
        const scriptPath = resolve(script);
        return runPreview({
            native,
            scriptPath,
            source: readFileSync(scriptPath, "utf8"),
            assetRoot: inferAssetRoot(scriptPath),
            outDir: join(dirname(scriptPath), "preview"),
        });
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

function resolveRenderTarget(options: ParsedOptions): {
    scriptPath: string;
    outPath: string;
    format: string;
    assetRoot: string;
    settings?: RenderSettingsInput;
} | null {
    const projectOpt = readString(options, "project");
    if (projectOpt) {
        const native = loadNative();
        const projectRoot = resolve(projectOpt);
        const manifest = loadManifest(native, projectRoot);
        const entry = resolveEntries(native, manifest, readString(options, "entry"))[0];
        if (!entry) {
            console.error("muse: project has no entries");
            return null;
        }
        const scriptPath = join(projectRoot, entry.source);
        const explicitOut = readString(options, "output");
        const outputId = readString(options, "outputId") ?? readString(options, "output-id");
        let format = (readString(options, "format") ?? "").toLowerCase();
        let outPath: string;
        let outputSpec: OutputSpec | undefined;
        if (explicitOut) {
            outPath = resolve(explicitOut);
            if (!format) format = extOf(outPath) || "mp4";
        } else if (outputId) {
            const spec = (entry.outputs ?? []).find((o) => o.id === outputId);
            if (!spec) {
                console.error(`muse: unknown output id \`${outputId}\` on entry \`${entry.id}\``);
                return null;
            }
            outputSpec = spec;
            outPath = join(projectRoot, spec.path);
            format =
                format ||
                (spec.format ?? "").toLowerCase() ||
                (spec.kind === "audio" ? "wav" : "mp4");
        } else {
            if (!format) format = "mp4";
            const kind = format === "mp4" ? "video" : "audio";
            const spec = (entry.outputs ?? []).find((o) => o.kind === kind);
            outputSpec = spec;
            outPath = spec
                ? join(projectRoot, spec.path)
                : join(
                      projectRoot,
                      "library",
                      entry.id,
                      `out.${format === "mp3" ? "mp3" : format === "wav" ? "wav" : "mp4"}`,
                  );
        }
        return {
            scriptPath,
            outPath,
            format,
            assetRoot: projectRoot,
            settings: resolveRenderSettings(manifest, entry, outputSpec),
        };
    }

    const script = requirePositional(options, "script");
    if (!script) return null;
    const output = readString(options, "output");
    if (!output) {
        console.error("muse: missing --output <file>");
        return null;
    }
    const scriptPath = resolve(script);
    const outPath = resolve(output);
    const format = (readString(options, "format") ?? (extOf(outPath) || "mp4")).toLowerCase();
    return {
        scriptPath,
        outPath,
        format,
        assetRoot: inferAssetRoot(scriptPath),
    };
}

export function forwardRender(options: ParsedOptions): number {
    try {
        const target = resolveRenderTarget(options);
        if (!target) return 2;
        const { scriptPath, outPath, format, assetRoot, settings } = target;
        const native = loadNative();
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
            const compiled = native.compileSource(source, settings, assetRoot);
            if (!compiled.ok || !compiled.plan) {
                printDiagnostics(compiled.diagnostics);
                return 1;
            }
            const plan = compiled.plan as PlanShape;
            const maxDuration =
                readNumber(options, "maxDuration") ?? readNumber(options, "max-duration");
            const planDuration = Math.max(0, plan.duration_seconds ?? 0);
            const encodeSeconds =
                maxDuration != null
                    ? Math.max(0, Math.min(maxDuration, planDuration || maxDuration))
                    : planDuration;
            const hasTracks = Array.isArray(plan.audio_tracks) && plan.audio_tracks.length > 0;
            const tempWav = join(tmpdir(), `muse-render-audio-${process.pid}-${Date.now()}.wav`);
            let audioWav: string | undefined;
            try {
                if (hasTracks) {
                    native.mixPlanAudioWav(compiled.plan, tempWav, encodeSeconds, assetRoot);
                    audioWav = tempWav;
                } else if (maxDuration == null) {
                    const score = native.composeScore(source);
                    native.writeScoreWav(score, tempWav);
                    audioWav = tempWav;
                }
                native.encodePlanMp4(compiled.plan, outPath, {
                    assetRoot,
                    ...(audioWav ? { audioWav } : {}),
                    ...(maxDuration != null ? { maxDurationSeconds: maxDuration } : {}),
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
        const project = readString(options, "project") ?? ".";
        const meta = native.importProjectAsset(resolve(project), resolve(file)) as {
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

export function forwardSkillList(options?: ParsedOptions): number {
    try {
        const project = options ? (readString(options, "project") ?? ".") : ".";
        const skills = loadNative().listProjectSkills(resolve(project)) as Array<{
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

export function forwardSkillValidate(options?: ParsedOptions): number {
    try {
        const project = options ? (readString(options, "project") ?? ".") : ".";
        const skills = loadNative().listProjectSkills(resolve(project)) as Array<{
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

export function forwardMcpServe(options?: ParsedOptions): number {
    try {
        const project = options ? (readString(options, "project") ?? ".") : ".";
        loadNative().serveMcpStdio(resolve(project));
        return 0;
    } catch (err) {
        console.error(`muse: ${err instanceof Error ? err.message : err}`);
        return 1;
    }
}

export { readFlag, readString };
