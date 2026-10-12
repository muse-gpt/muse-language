"use strict";

const { existsSync } = require("node:fs");
const { join } = require("node:path");

/** Absolute path to the native `muse` binary shipped in this package. */
const bin = join(__dirname, "lib", "muse");

/**
 * Resolve the native binary path, or throw if the file is missing from the install.
 * `@muse-gpt/muse` should call this (or read `bin`) instead of inventing paths.
 */
function resolve() {
    if (!existsSync(bin)) {
        throw new Error(`@muse-gpt/muse-darwin-x64: native binary missing at ${bin}`);
    }
    return bin;
}

module.exports = { bin, resolve };
module.exports.default = bin;
