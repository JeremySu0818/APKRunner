import { contextBridge, ipcRenderer } from "electron";
import { IPC_CHANNELS } from "../shared/ipcChannels";
import type { APKRunnerPreloadApi } from "../shared/protocol";

const api: APKRunnerPreloadApi = {
  openApk: () => ipcRenderer.invoke(IPC_CHANNELS.openApk),
  getApkInfo: () => ipcRenderer.invoke(IPC_CHANNELS.getApkInfo),
  startApp: () => ipcRenderer.invoke(IPC_CHANNELS.startApp),
  stopApp: () => ipcRenderer.invoke(IPC_CHANNELS.stopApp),
  getStatus: () => ipcRenderer.invoke(IPC_CHANNELS.getStatus),
  pollEvents: () => ipcRenderer.invoke(IPC_CHANNELS.pollEvents)
};

contextBridge.exposeInMainWorld("APKRunner", api);
