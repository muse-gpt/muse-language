"use strict";

const { existsSync } = require("node:fs");
const { join } = require("node:path");

const addon = join(__dirname, "lib", "muse_napi.node");
const pkg = require("./package.json");

function resolveAddon() {
    if (!existsSync(addon)) {
        throw new Error(pkg.name + ": muse-napi addon missing at " + addon);
    }
    return addon;
}

function resolve() {
    return resolveAddon();
}

module.exports = { addon, resolveAddon, resolve };
module.exports.default = addon;
