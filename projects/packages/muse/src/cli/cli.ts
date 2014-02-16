import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

import { createCli } from "@vmz/commander";

import { registerAssetCommand } from "./commands/asset-cmd.js";
import { registerCheckCommand } from "./commands/check-cmd.js";
import { registerCompileCommand } from "./commands/compile-cmd.js";
import { registerDoctorCommand } from "./commands/doctor-cmd.js";
import { registerExportCommand } from "./commands/export-cmd.js";
import { registerInitCommand } from "./commands/init-cmd.js";
import { registerMcpCommand } from "./commands/mcp-cmd.js";
import { registerPreviewCommand } from "./commands/preview-cmd.js";
import { registerRenderCommand } from "./commands/render-cmd.js";
import { registerScoreCommand } from "./commands/score-cmd.js";
import { registerSkillCommand } from "./commands/skill-cmd.js";

export function localesRoot(): string {
    return join(dirname(fileURLToPath(import.meta.url)), "../../locales");
}

/** Build the `muse` command tree (@vmz/commander). */
export function buildMuseCli() {
    const root = localesRoot();
    const cli = createCli("muse")
        .locales(root, { envKeys: ["MUSE_LOCALE", "LOCALE", "LANG", "LC_ALL"] })
        .intro("cli.intro");

    registerInitCommand(cli);
    registerCheckCommand(cli);
    registerCompileCommand(cli);
    registerPreviewCommand(cli);
    registerRenderCommand(cli);
    registerExportCommand(cli);
    registerAssetCommand(cli);
    registerScoreCommand(cli);
    registerSkillCommand(cli);
    registerMcpCommand(cli);
    registerDoctorCommand(cli);

    return cli;
}
