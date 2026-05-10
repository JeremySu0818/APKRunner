import { app } from "electron";
import { mkdirSync } from "node:fs";
import { join } from "node:path";

export interface AppDataPaths {
  root: string;
  sandboxRoot: string;
  androidRuntimeRoot: string;
}

export function getAppDataPaths(): AppDataPaths {
  const root = app.getPath("userData");
  const sandboxRoot = join(root, "android-sandbox");
  const androidRuntimeRoot = join(root, "android-runtime");
  mkdirSync(sandboxRoot, { recursive: true });
  mkdirSync(androidRuntimeRoot, { recursive: true });
  return { root, sandboxRoot, androidRuntimeRoot };
}
