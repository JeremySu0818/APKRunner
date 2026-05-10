import { app } from "electron";
import { createRequire } from "node:module";
import { existsSync } from "node:fs";
import { basename, dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import type {
  ApkSummary,
  InputEvent,
  RuntimeBundleStatus,
  RuntimeEvent,
  RuntimeOperationStatus
} from "../shared/protocol";

interface NativeRunnerCreated {
  runnerId: string;
  backendName: string;
}

interface NativeAppInstanceCreated {
  instanceId: string;
}

export interface NativeAddon {
  createRunner(config: Record<string, unknown>): NativeRunnerCreated;
  loadApk(runnerId: string, apkPath: string): ApkSummary;
  createAppInstance(runnerId: string, config: Record<string, unknown>): NativeAppInstanceCreated;
  startApp(runnerId: string, instanceId: string): Record<string, unknown>;
  stopApp(runnerId: string, instanceId: string): Record<string, unknown>;
  dispatchInput(runnerId: string, instanceId: string, input: InputEvent): Record<string, unknown>;
  getRuntimeBundleStatus(config: Record<string, unknown>): RuntimeBundleStatus;
  startRuntimeDownload(config: Record<string, unknown>): RuntimeOperationStatus;
  startRuntimeDelete(config: Record<string, unknown>): RuntimeOperationStatus;
  getRuntimeOperationStatus(operationId: string): RuntimeOperationStatus;
  pollEvents(runnerId: string): RuntimeEvent[];
}

export interface NativeLoadState {
  available: boolean;
  addon: NativeAddon | null;
  attemptedPaths: string[];
  error: string | null;
}

const require = createRequire(import.meta.url);
let cachedState: NativeLoadState | null = null;

function currentModuleDir(): string {
  return dirname(fileURLToPath(import.meta.url));
}

function unique(values: Array<string | undefined>): string[] {
  return [...new Set(values.filter((value): value is string => Boolean(value)))];
}

function candidatePaths(): string[] {
  const cwd = process.cwd();
  const appPath = app.getAppPath();
  const moduleDir = currentModuleDir();
  const platformExt = process.platform === "win32" ? "dll" : process.platform === "darwin" ? "dylib" : "so";
  const cargoLibName = process.platform === "win32" ? "apkrunner_napi.dll" : `libapkrunner_napi.${platformExt}`;

  return unique([
    process.env.APKRUNNER_NATIVE_PATH,
    resolve(cwd, "apps/desktop/native/apkrunner_napi.node"),
    resolve(cwd, "native/apkrunner_napi.node"),
    resolve(cwd, "target/release/apkrunner_napi.node"),
    resolve(cwd, "target/debug/apkrunner_napi.node"),
    resolve(cwd, "target/release", cargoLibName),
    resolve(cwd, "target/debug", cargoLibName),
    resolve(appPath, "native/apkrunner_napi.node"),
    resolve(moduleDir, "../native/apkrunner_napi.node")
  ]);
}

function describeError(path: string, error: unknown): string {
  const message = error instanceof Error ? error.stack ?? error.message : String(error);
  return `${basename(path)}: ${message}`;
}

export function loadNativeAddon(): NativeLoadState {
  if (cachedState) {
    return cachedState;
  }

  const attemptedPaths = candidatePaths();
  const failures: string[] = [];

  for (const path of attemptedPaths) {
    try {
      if (!existsSync(path)) {
        failures.push(`${path}: file does not exist`);
        continue;
      }
      const addon = require(path) as NativeAddon;
      cachedState = {
        available: true,
        addon,
        attemptedPaths,
        error: null
      };
      console.info(`[APKRunner] Loaded native addon from ${path}`);
      return cachedState;
    } catch (error) {
      const detail = describeError(path, error);
      failures.push(`${path}: ${detail}`);
      console.error(`[APKRunner] Failed to load native addon from ${path}`, error);
    }
  }

  cachedState = {
    available: false,
    addon: null,
    attemptedPaths,
    error: failures.join("\n")
  };
  return cachedState;
}
