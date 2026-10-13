/**
 * Local helper: copy built muse-napi into the matching platform package lib/.
 * Usage (repo root): node scripts/stage-native.mjs
 */

import { spawnSync } from "node:child_process";
import path from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const r = spawnSync(process.execPath, [path.join(ROOT, "scripts/copy-napi-addon.mjs")], {
    cwd: ROOT,
    stdio: "inherit",
});
process.exit(r.status ?? 1);
