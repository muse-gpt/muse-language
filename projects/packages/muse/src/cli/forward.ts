import type { ParsedOptions } from "@vmz/commander";

import { runNative } from "./native.js";

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

export function forwardCheck(options: ParsedOptions): number {
    const script = requirePositional(options, "script");
    if (!script) return 2;
    return runNative(["check", script]);
}

export function forwardInit(options: ParsedOptions): number {
    const path = options._[0];
    const args = ["init"];
    if (typeof path === "string" && path.length > 0) args.push(path);
    return runNative(args);
}

export function forwardCompile(options: ParsedOptions): number {
    const script = requirePositional(options, "script");
    if (!script) return 2;
    const args = ["compile", script];
    const emit = readString(options, "emit");
    if (emit) args.push("--emit", emit);
    return runNative(args);
}

export function forwardPreview(options: ParsedOptions): number {
    const script = requirePositional(options, "script");
    if (!script) return 2;
    return runNative(["preview", script]);
}

export function forwardRender(options: ParsedOptions): number {
    const script = requirePositional(options, "script");
    if (!script) return 2;
    const output = readString(options, "output");
    if (!output) {
        console.error("muse: missing --output");
        return 2;
    }
    const args = ["render", script, "--output", output];
    const format = readString(options, "format");
    if (format) args.push("--format", format);
    return runNative(args);
}

export function forwardExport(options: ParsedOptions): number {
    const script = requirePositional(options, "script");
    if (!script) return 2;
    const output = readString(options, "output");
    const format = readString(options, "format");
    if (!output || !format) {
        console.error("muse: export requires --output and --format");
        return 2;
    }
    return runNative(["export", script, "--output", output, "--format", format]);
}

export function forwardAssetImport(options: ParsedOptions): number {
    const file = requirePositional(options, "file");
    if (!file) return 2;
    return runNative(["asset", "import", file]);
}

export function forwardAssetInfo(options: ParsedOptions): number {
    const file = requirePositional(options, "file");
    if (!file) return 2;
    return runNative(["asset", "info", file]);
}

export function forwardScoreInfo(options: ParsedOptions): number {
    const file = requirePositional(options, "file");
    if (!file) return 2;
    return runNative(["score", "info", file]);
}

export function forwardSkillList(): number {
    return runNative(["skill", "list"]);
}

export function forwardSkillValidate(): number {
    return runNative(["skill", "validate"]);
}

export function forwardMcpServe(): number {
    return runNative(["mcp", "serve"]);
}

export { readFlag, readString };
