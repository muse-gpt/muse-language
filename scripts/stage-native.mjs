/**
 * Local helper: copy target/release/muse(.exe) into the matching platform package lib/.
 * Usage (repo root): node scripts/stage-native.mjs
 */

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");

const plat =
    process.platform === "win32"
        ? "win32-x64"
        : process.platform === "darwin"
          ? process.arch === "arm64"
              ? "darwin-arm64"
              : "darwin-x64"
          : process.arch === "arm64"
            ? "linux-arm64"
            : "linux-x64";

const exe = process.platform === "win32" ? "muse.exe" : "muse";
const src = path.join(ROOT, "target", "release", exe);
const destDir = path.join(ROOT, "projects", "packages", `muse-${plat}`, "lib");
const dest = path.join(destDir, exe);

if (!fs.existsSync(src)) {
    console.error(`stage-native: missing ${src}; run cargo build --release -p muse-tools first`);
    process.exit(1);
}

fs.mkdirSync(destDir, { recursive: true });
fs.copyFileSync(src, dest);
console.log(`stage-native: ${src} -> ${dest}`);
