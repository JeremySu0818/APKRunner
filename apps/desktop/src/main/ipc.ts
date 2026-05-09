import { ipcMain } from "electron";
import { IPC_CHANNELS } from "../shared/ipcChannels";
import type { IpcError, IpcResult } from "../shared/protocol";
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

function registerHandler<T>(channel: string, handler: () => T | Promise<T>): void {
  ipcMain.handle(channel, async (_event, ...payload: unknown[]): Promise<IpcResult<T>> => {
    try {
      if (payload.length > 0) {
        throw new Error(`Unexpected payload for IPC channel ${channel}.`);
      }
      return ok(await handler());
    } catch (error) {
      return fail(error);
    }
  });
}

export function registerIpcHandlers(service: RuntimeService): void {
  registerHandler(IPC_CHANNELS.openApk, () => service.openApk());
  registerHandler(IPC_CHANNELS.getApkInfo, () => service.apkInfo());
  registerHandler(IPC_CHANNELS.startApp, () => service.startApp());
  registerHandler(IPC_CHANNELS.stopApp, () => service.stopApp());
  registerHandler(IPC_CHANNELS.getStatus, () => service.status());
  registerHandler(IPC_CHANNELS.pollEvents, () => service.pollEvents());
}
