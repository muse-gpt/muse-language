import type { Cli, ParsedOptions } from "@vmz/commander";

import { forwardAssetImport, forwardAssetInfo } from "../forward.js";

export function registerAssetCommand(cli: Cli): void {
    const asset = cli.command("asset", "cli.cmd.asset");
    asset
        .command("import", "cli.cmd.asset.import")
        .option("--project <dir>", "cli.opt.project")
        .action((options: ParsedOptions) => forwardAssetImport(options));
    asset.command("info", "cli.cmd.asset.info").action((options: ParsedOptions) =>
        forwardAssetInfo(options),
    );
}
