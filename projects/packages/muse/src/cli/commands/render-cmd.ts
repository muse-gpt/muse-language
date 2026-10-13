import type { Cli, ParsedOptions } from "@vmz/commander";

import { forwardRender } from "../forward.js";

export function registerRenderCommand(cli: Cli): void {
    cli.command("render", "cli.cmd.render")
        .option("-o, --output <file>", "cli.opt.output")
        .option("--format <format>", "cli.opt.render-format")
        .option("--project <dir>", "cli.opt.project")
        .option("--entry <id>", "cli.opt.entry")
        .option("--output-id <id>", "cli.opt.output-id")
        .option("--max-duration <seconds>", "cli.opt.max-duration")
        .action((options: ParsedOptions) => forwardRender(options));
}
