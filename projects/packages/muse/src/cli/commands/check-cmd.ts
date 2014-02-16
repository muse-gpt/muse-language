import type { Cli, ParsedOptions } from "@vmz/commander";

import { forwardCheck } from "../forward.js";

export function registerCheckCommand(cli: Cli): void {
    cli.command("check", "cli.cmd.check").action((options: ParsedOptions) => forwardCheck(options));
}
