import { existsSync } from "node:fs";
import { delimiter, join } from "node:path";

import type { Cli, ParsedOptions } from "@vmz/commander";

import { resolveNativeMuse } from "../native.js";

type DoctorCheck = { id: string; ok: boolean; message: string };

export function registerDoctorCommand(cli: Cli): void {
    cli.command("doctor", "cli.cmd.doctor")
        .option("--json", "cli.opt.json")
        .action((options: ParsedOptions) => runDoctor(options));
}

function resolveFfmpeg(): string | undefined {
    const explicit = process.env.MUSE_FFMPEG?.trim();
    if (explicit && existsSync(explicit)) {
        return explicit;
    }
    const pathEnv = process.env.PATH ?? process.env.Path ?? "";
    const exts = process.platform === "win32" ? [".exe", ""] : [""];
    for (const dir of pathEnv.split(delimiter)) {
        if (!dir) continue;
        for (const ext of exts) {
            const candidate = join(dir, `ffmpeg${ext}`);
            if (existsSync(candidate)) return candidate;
        }
    }
    return undefined;
}

function runDoctor(options: ParsedOptions): number {
    const checks: DoctorCheck[] = [];
    checks.push({
        id: "node",
        ok: Number(process.versions.node.split(".")[0]) >= 20,
        message: `node ${process.version}`,
    });

    const native = resolveNativeMuse();
    checks.push({
        id: "native",
        ok: Boolean(native),
        message: native ? `muse binary: ${native}` : "native muse binary not found",
    });

    const ffmpeg = resolveFfmpeg();
    checks.push({
        id: "ffmpeg",
        ok: Boolean(ffmpeg),
        message: ffmpeg
            ? `ffmpeg for mp4: ${ffmpeg}`
            : "ffmpeg not found (optional; required for `muse render --format mp4`)",
    });

    // ffmpeg is optional — doctor stays green without it.
    const ok = checks.filter((c) => c.id !== "ffmpeg").every((c) => c.ok);
    if (options.json === true) {
        console.log(JSON.stringify({ ok, checks }, null, 2));
    } else {
        console.log(ok ? "muse doctor: ok" : "muse doctor: issues found");
        for (const c of checks) {
            console.log(`  ${c.ok ? "ok" : "FAIL"}  ${c.id}: ${c.message}`);
        }
    }
    return ok ? 0 : 1;
}
