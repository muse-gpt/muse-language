import type { Cli, ParsedOptions } from "@vmz/commander";

import { forwardScoreInfo } from "../forward.js";

export function registerScoreCommand(cli: Cli): void {
    const score = cli.command("score", "cli.cmd.score");
    score
        .command("info", "cli.cmd.score.info")
        .action((options: ParsedOptions) => forwardScoreInfo(options));
}
