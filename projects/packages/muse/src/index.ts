/** Public Node entry for `@muse/muse`. Prefer the `muse` bin → @vmz/commander → native Rust. */
export const name = "@muse/muse";
export const version = "0.0.1";

export { buildMuseCli, runCli, localesRoot, resolveNativeMuse, runNative } from "./cli/index.js";
