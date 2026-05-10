import { ipcMain } from "electron";
import { IPC_CHANNELS } from "../shared/ipcChannels";
import type { InputEvent, IpcError, IpcResult } from "../shared/protocol";
import { RuntimeService } from "./runtimeService";

function normalizeError(error: unknown): IpcError {
  if (error instanceof Error) {
    return {
      code: "APKRUNNER_ERROR",
      message: error.message,
      details: error.stack
    };
  }
  return {
    code: "APKRUNNER_ERROR",
    message: String(error)
  };
}

function ok<T>(data: T): IpcResult<T> {
  return { success: true, data };
}

function fail(error: unknown): IpcResult<never> {
  return { success: false, error: normalizeError(error) };
}

function registerHandler<T>(
  channel: string,
  handler: (...payload: unknown[]) => T | Promise<T>,
  options: { allowPayload?: boolean } = {}
): void {
  ipcMain.handle(channel, async (_event, ...payload: unknown[]): Promise<IpcResult<T>> => {
    try {
      if (!options.allowPayload && payload.length > 0) {
        throw new Error(`Unexpected payload for IPC channel ${channel}.`);
      }
      return ok(await handler(...payload));
    } catch (error) {
      return fail(error);
    }
  });
}

function isInputEvent(value: unknown): value is InputEvent {
  if (!value || typeof value !== "object") {
    return false;
  }
  const event = value as Record<string, unknown>;
  if (event.type === "tap") {
    return Number.isInteger(event.x) && Number.isInteger(event.y);
  }
  if (event.type === "key") {
    return Number.isInteger(event.keyCode);
  }
  if (event.type === "text") {
    return typeof event.text === "string";
  }
  return false;
}

function requireInputEvent(value: unknown): InputEvent {
  if (!isInputEvent(value)) {
    throw new Error("Invalid input event payload.");
  }
  return value;
}

function requireString(value: unknown, label: string): string {
  if (typeof value !== "string" || value.length === 0) {
    throw new Error(`Invalid ${label}.`);
  }
  return value;
}

export function registerIpcHandlers(service: RuntimeService): void {
  registerHandler(IPC_CHANNELS.openApk, () => service.openApk());
  registerHandler(IPC_CHANNELS.getApkInfo, () => service.apkInfo());
  registerHandler(IPC_CHANNELS.startApp, () => service.startApp());
  registerHandler(IPC_CHANNELS.stopApp, () => service.stopApp());
  registerHandler(
    IPC_CHANNELS.dispatchInput,
    (payload) => service.dispatchInput(requireInputEvent(payload)),
    { allowPayload: true }
  );
  registerHandler(IPC_CHANNELS.getRuntimeBundleStatus, () => service.runtimeBundleStatus());
  registerHandler(IPC_CHANNELS.startRuntimeDownload, () => service.startRuntimeDownload());
  registerHandler(IPC_CHANNELS.startRuntimeDelete, () => service.startRuntimeDelete());
  registerHandler(
    IPC_CHANNELS.getRuntimeOperationStatus,
    (payload) => service.runtimeOperationStatus(requireString(payload, "runtime operation id")),
    { allowPayload: true }
  );
  registerHandler(IPC_CHANNELS.getStatus, () => service.status());
  registerHandler(IPC_CHANNELS.pollEvents, () => service.pollEvents());
}
