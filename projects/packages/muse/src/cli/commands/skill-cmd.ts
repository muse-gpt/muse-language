import type { Cli } from "@vmz/commander";

import { forwardSkillList, forwardSkillValidate } from "../forward.js";

export function registerSkillCommand(cli: Cli): void {
    const skill = cli.command("skill", "cli.cmd.skill");
    skill.command("list", "cli.cmd.skill.list").action(() => forwardSkillList());
    skill.command("validate", "cli.cmd.skill.validate").action(() => forwardSkillValidate());
}
