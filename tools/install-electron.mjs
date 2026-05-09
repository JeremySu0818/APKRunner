import { spawn } from "node:child_process";
import { join } from "node:path";

const repoRoot = join(import.meta.dirname, "..");
const installScript = join(repoRoot, "node_modules", "electron", "install.js");

const env = {
  ...process.env,
  ELECTRON_MIRROR: process.env.ELECTRON_MIRROR ?? "https://cdn.npmmirror.com/binaries/electron/",
  ELECTRON_CUSTOM_DIR: process.env.ELECTRON_CUSTOM_DIR ?? "{{ version }}"
};

const child = spawn(process.execPath, [installScript], {
  cwd: repoRoot,
  env,
  stdio: "inherit"
});

child.on("exit", (code, signal) => {
  if (signal) {
    process.kill(process.pid, signal);
    return;
  }

  process.exit(code ?? 1);
});
