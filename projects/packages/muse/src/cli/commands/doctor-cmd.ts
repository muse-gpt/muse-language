import type { Cli, ParsedOptions } from "@vmz/commander";

import { tryLoadNative } from "../../native.js";

type DoctorCheck = { id: string; ok: boolean; message: string };

export function registerDoctorCommand(cli: Cli): void {
    cli.command("doctor", "cli.cmd.doctor")
        .option("--json", "cli.opt.json")
        .action((options: ParsedOptions) => {
            process.exitCode = runDoctor(options);
        });
}

function runDoctor(options: ParsedOptions): number {
    const checks: DoctorCheck[] = [];
    const native = tryLoadNative();
    if (!native) {
        checks.push({
            id: "napi",
            ok: false,
            message: "muse-napi addon missing (build `muse-napi`, expect muse_napi.node)",
        });
    } else {
        checks.push({ id: "napi", ok: true, message: "muse-napi loaded" });
        try {
            const gpu = native.probeGpuStatus();
            checks.push({
                id: "gpu",
                ok: gpu.status !== "unavailable",
                message: gpu.adapter_name
                    ? `${gpu.message} (${gpu.adapter_name})`
                    : gpu.message,
            });
            checks.push({
                id: "compositor",
                ok: true,
                message: `preferred ${gpu.preferred}`,
            });
        } catch (err) {
            checks.push({
                id: "gpu",
                ok: false,
                message: err instanceof Error ? err.message : String(err),
            });
        }
        const ffmpeg = native.findFfmpegPath();
        checks.push({
            id: "ffmpeg",
            ok: Boolean(ffmpeg),
            message: ffmpeg ?? "ffmpeg not found on PATH (optional for encode)",
        });
    }

    const ok = checks.every((c) => c.ok || c.id === "ffmpeg");
    const json = options.json === true;
    if (json) {
        console.log(JSON.stringify({ ok, checks }, null, 2));
    } else {
        for (const c of checks) {
            console.log(`${c.ok ? "ok" : "!!"} ${c.id}: ${c.message}`);
        }
    }
    return ok ? 0 : 1;
}
