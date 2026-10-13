import type { Cli, ParsedOptions } from "@vmz/commander";

import { forwardPreview } from "../forward.js";

export function registerPreviewCommand(cli: Cli): void {
    cli.command("preview", "cli.cmd.preview")
        .option("--project <dir>", "cli.opt.project")
        .option("--entry <id>", "cli.opt.entry")
        .option("--job <id>", "cli.opt.job")
        .option("--max-frames <n>", "cli.opt.max-frames")
        .action((options: ParsedOptions) => forwardPreview(options));
}
