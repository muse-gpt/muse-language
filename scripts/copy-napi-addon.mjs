import { copyFileSync, existsSync, mkdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const profile = process.argv.includes("--debug") ? "debug" : "release";

const candidates =
    process.platform === "win32"
        ? [`muse_napi.dll`]
        : process.platform === "darwin"
          ? [`libmuse_napi.dylib`, `muse_napi.dylib`]
          : [`libmuse_napi.so`, `muse_napi.so`];

const dir = join(root, "target", profile);
let src = null;
for (const name of candidates) {
    const path = join(dir, name);
    if (existsSync(path)) {
        src = path;
        break;
    }
}
if (!src) {
    console.error(`copy-napi-addon: no addon found under ${dir}`);
    process.exit(1);
}

const dests = [join(dir, "muse_napi.node")];
if (process.platform === "win32") {
    const platLib = join(root, "projects/packages/muse-win32-x64/lib");
    mkdirSync(platLib, { recursive: true });
    dests.push(join(platLib, "muse_napi.node"));
}

for (const dest of dests) {
    copyFileSync(src, dest);
    console.log(`copied ${src} -> ${dest}`);
}
