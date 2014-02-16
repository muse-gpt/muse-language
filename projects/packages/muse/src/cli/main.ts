import { buildMuseCli } from "./cli.js";

export async function runCli(argv: string[]): Promise<number> {
    try {
        return await buildMuseCli().parse(argv);
    } catch (error) {
        console.error(`muse: ${error instanceof Error ? error.message : String(error)}`);
        return 2;
    }
}
