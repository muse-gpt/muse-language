/** @deprecated Prefer `../native.js` — kept as re-export for existing relative imports. */
export {
    loadNative,
    tryLoadNative,
    type MuseNative,
    type CompileResult,
    type ParseResult,
    type Diagnostic,
    type RenderSettingsInput,
} from "../native.js";

/** @deprecated CLI no longer spawns a Rust binary. */
export function resolveNativeMuse(): string | null {
    return null;
}

/** @deprecated */
export function runNative(_argv: string[]): number {
    console.error(
        "muse: native CLI binary removed; commands compose muse-napi primitives. Rebuild muse-napi if load fails.",
    );
    return 1;
}
