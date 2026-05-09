import { app } from "electron";
import { mkdirSync } from "node:fs";
import { join } from "node:path";

export interface AppDataPaths {
  root: string;
  sandboxRoot: string;
}

export function getAppDataPaths(): AppDataPaths {
  const root = app.getPath("userData");
  const sandboxRoot = join(root, "android-sandbox");
  mkdirSync(sandboxRoot, { recursive: true });
  return { root, sandboxRoot };
}
