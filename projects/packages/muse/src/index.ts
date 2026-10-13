/** Public Node entry for `@muse-gpt/muse`. */
export const name = "@muse-gpt/muse";
export const version = "0.0.1";

export {
    loadNative,
    tryLoadNative,
    type MuseNative,
    type CompileResult,
    type ParseResult,
    type Diagnostic,
    type RenderSettingsInput,
} from "./native.js";

export { buildMuseCli, runCli, localesRoot } from "./cli/index.js";
