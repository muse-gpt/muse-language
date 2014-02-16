import type { Cli, ParsedOptions } from "@vmz/commander";

import { forwardCompile } from "../forward.js";

export function registerCompileCommand(cli: Cli): void {
    cli.command("compile", "cli.cmd.compile")
        .option("--emit <format>", "cli.opt.emit")
        .action((options: ParsedOptions) => forwardCompile(options));
}
