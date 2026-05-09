import { copyFile, mkdir } from "node:fs/promises";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = join(dirname(fileURLToPath(import.meta.url)), "..");
const platform = process.platform;

const sourceName =
  platform === "win32"
    ? "apkrunner_napi.dll"
    : platform === "darwin"
      ? "libapkrunner_napi.dylib"
      : "libapkrunner_napi.so";

const source = join(repoRoot, "target", "release", sourceName);
const destination = join(repoRoot, "apps", "desktop", "native", "apkrunner_napi.node");

await mkdir(dirname(destination), { recursive: true });
await copyFile(source, destination);
console.log(`Copied ${source} -> ${destination}`);
