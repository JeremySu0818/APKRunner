import { readdir, readFile } from "node:fs/promises";
import { dirname, join, relative } from "node:path";
import { builtinModules } from "node:module";
import { fileURLToPath } from "node:url";

const repoRoot = join(dirname(fileURLToPath(import.meta.url)), "..");
const rendererRoot = join(repoRoot, "apps", "desktop", "src", "renderer");
const forbidden = new Set([
  "electron",
  ...builtinModules,
  ...builtinModules.map((name) => `node:${name}`)
]);

async function* walk(dir) {
  for (const entry of await readdir(dir, { withFileTypes: true })) {
    const path = join(dir, entry.name);
    if (entry.isDirectory()) {
      yield* walk(path);
    } else if (/\.(ts|tsx)$/.test(entry.name)) {
      yield path;
    }
  }
}

const importPattern = /\bimport\s+(?:type\s+)?(?:[^'"]+\s+from\s+)?["']([^"']+)["']|\bexport\s+(?:type\s+)?[^'"]+\s+from\s+["']([^"']+)["']/g;
const failures = [];

for await (const file of walk(rendererRoot)) {
  const source = await readFile(file, "utf8");
  for (const match of source.matchAll(importPattern)) {
    const specifier = match[1] ?? match[2];
    if (forbidden.has(specifier)) {
      failures.push(`${relative(repoRoot, file)} imports forbidden renderer module "${specifier}"`);
    }
  }
}

if (failures.length > 0) {
  console.error(failures.join("\n"));
  process.exitCode = 1;
}
