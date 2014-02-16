import type { Cli, ParsedOptions } from "@vmz/commander";

import { forwardInit } from "../forward.js";

export function registerInitCommand(cli: Cli): void {
    cli.command("init", "cli.cmd.init").action((options: ParsedOptions) => forwardInit(options));
}
