import { existsSync } from "node:fs";
import { delimiter, join } from "node:path";
import { spawnSync } from "node:child_process";

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

function runNativeDoctor(): { ok: boolean; checks: DoctorCheck[] } | undefined {
    const native = resolveNativeMuse();
    if (!native) return undefined;
    const r = spawnSync(native, ["doctor", "--json"], {
        encoding: "utf8",
        shell: false,
    });
    if (r.error || typeof r.stdout !== "string" || !r.stdout.trim()) {
        return undefined;
    }
    try {
        const report = JSON.parse(r.stdout) as {
            ok?: boolean;
            compositor?: string;
            gpu?: { status?: string; message?: string; adapter_name?: string };
            ffmpeg?: { ok?: boolean; message?: string };
        };
        const checks: DoctorCheck[] = [];
        const gpuMsg = report.gpu?.adapter_name
            ? `${report.gpu.message ?? "gpu"} (${report.gpu.adapter_name})`
            : (report.gpu?.message ?? "gpu probe missing");
        checks.push({
            id: "gpu",
            ok: report.gpu?.status !== "unavailable",
            message: gpuMsg,
        });
        checks.push({
            id: "compositor",
            ok: true,
            message: `preferred ${report.compositor ?? "?"}`,
        });
        if (report.ffmpeg) {
            checks.push({
                id: "ffmpeg",
                ok: Boolean(report.ffmpeg.ok),
                message: report.ffmpeg.message ?? "ffmpeg",
            });
        }
        return { ok: report.ok !== false, checks };
    } catch {
        return undefined;
    }
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

    const nativeDoctor = runNativeDoctor();
    if (nativeDoctor) {
        checks.push(...nativeDoctor.checks);
    } else {
        const ffmpeg = resolveFfmpeg();
        checks.push({
            id: "ffmpeg",
            ok: Boolean(ffmpeg),
            message: ffmpeg
                ? `ffmpeg for encode: ${ffmpeg}`
                : "ffmpeg not found (optional; encode-only for mp4)",
        });
        checks.push({
            id: "gpu",
            ok: false,
            message: "native muse doctor unavailable (build muse-tools to probe GPU)",
        });
    }

    // node + native required; ffmpeg optional; gpu unavailable is soft unless native said fail
    const required = checks.filter((c) => c.id === "node" || c.id === "native");
    const ok =
        required.every((c) => c.ok) && (nativeDoctor ? nativeDoctor.ok : true);
    if (options.json === true) {
        console.log(JSON.stringify({ ok, checks }, null, 2));
    } else {
        console.log(ok ? "muse doctor: ok" : "muse doctor: issues found");
        for (const c of checks) {
            const soft = c.id === "ffmpeg";
            const label = c.ok ? "ok" : soft ? "warn" : "FAIL";
            console.log(`  ${label}  ${c.id}: ${c.message}`);
        }
    }
    return ok ? 0 : 1;
}
