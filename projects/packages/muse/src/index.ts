/** Public Node entry for `@muse-gpt/muse`. Prefer the `muse` bin for end users. */
export const name = "@muse-gpt/muse";
export const version = "0.0.0";

export { buildMuseCli, runCli, localesRoot, resolveNativeMuse, runNative } from "./cli/index.js";
