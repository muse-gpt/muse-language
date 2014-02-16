import type { Cli, ParsedOptions } from "@vmz/commander";

import { forwardPreview } from "../forward.js";

export function registerPreviewCommand(cli: Cli): void {
    cli.command("preview", "cli.cmd.preview").action((options: ParsedOptions) =>
        forwardPreview(options),
    );
}
