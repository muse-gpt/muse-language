import type { Cli, ParsedOptions } from "@vmz/commander";

import { resolveNativeMuse } from "../native.js";

type DoctorCheck = { id: string; ok: boolean; message: string };

export function registerDoctorCommand(cli: Cli): void {
    cli.command("doctor", "cli.cmd.doctor")
        .option("--json", "cli.opt.json")
        .action((options: ParsedOptions) => runDoctor(options));
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

    const ok = checks.every((c) => c.ok);
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
