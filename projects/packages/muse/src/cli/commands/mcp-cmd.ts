import type { Cli } from "@vmz/commander";

import { forwardMcpServe } from "../forward.js";

export function registerMcpCommand(cli: Cli): void {
    const mcp = cli.command("mcp", "cli.cmd.mcp");
    mcp.command("serve", "cli.cmd.mcp.serve").action(() => forwardMcpServe());
}
