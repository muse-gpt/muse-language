import type { Cli, ParsedOptions } from "@vmz/commander";

import { forwardSkillList, forwardSkillValidate } from "../forward.js";

export function registerSkillCommand(cli: Cli): void {
    const skill = cli.command("skill", "cli.cmd.skill");
    skill
        .command("list", "cli.cmd.skill.list")
        .option("--project <dir>", "cli.opt.project")
        .action((options: ParsedOptions) => forwardSkillList(options));
    skill
        .command("validate", "cli.cmd.skill.validate")
        .option("--project <dir>", "cli.opt.project")
        .action((options: ParsedOptions) => forwardSkillValidate(options));
}
