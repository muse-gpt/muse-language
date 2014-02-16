import type { Cli, ParsedOptions } from "@vmz/commander";

import { forwardExport } from "../forward.js";

export function registerExportCommand(cli: Cli): void {
    cli.command("export", "cli.cmd.export")
        .option("-o, --output <file>", "cli.opt.output")
        .option("--format <format>", "cli.opt.export-format")
        .action((options: ParsedOptions) => forwardExport(options));
}
