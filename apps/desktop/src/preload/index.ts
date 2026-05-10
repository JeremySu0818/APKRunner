import { contextBridge, ipcRenderer } from "electron";
import { IPC_CHANNELS } from "../shared/ipcChannels";
import type { APKRunnerPreloadApi } from "../shared/protocol";

const api: APKRunnerPreloadApi = {
  openApk: () => ipcRenderer.invoke(IPC_CHANNELS.openApk),
  getApkInfo: () => ipcRenderer.invoke(IPC_CHANNELS.getApkInfo),
  startApp: () => ipcRenderer.invoke(IPC_CHANNELS.startApp),
  stopApp: () => ipcRenderer.invoke(IPC_CHANNELS.stopApp),
  dispatchInput: (input) => ipcRenderer.invoke(IPC_CHANNELS.dispatchInput, input),
  getRuntimeBundleStatus: () => ipcRenderer.invoke(IPC_CHANNELS.getRuntimeBundleStatus),
  startRuntimeDownload: () => ipcRenderer.invoke(IPC_CHANNELS.startRuntimeDownload),
  startRuntimeDelete: () => ipcRenderer.invoke(IPC_CHANNELS.startRuntimeDelete),
  getRuntimeOperationStatus: (operationId) =>
    ipcRenderer.invoke(IPC_CHANNELS.getRuntimeOperationStatus, operationId),
  getStatus: () => ipcRenderer.invoke(IPC_CHANNELS.getStatus),
  pollEvents: () => ipcRenderer.invoke(IPC_CHANNELS.pollEvents)
};

contextBridge.exposeInMainWorld("APKRunner", api);
