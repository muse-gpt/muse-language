import type { Cli, ParsedOptions } from "@vmz/commander";

import { forwardCheck } from "../forward.js";

export function registerCheckCommand(cli: Cli): void {
    cli.command("check", "cli.cmd.check")
        .option("--project <dir>", "cli.opt.project")
        .option("--entry <id>", "cli.opt.entry")
        .option("--job <id>", "cli.opt.job")
        .action((options: ParsedOptions) => forwardCheck(options));
}
