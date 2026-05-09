export const IPC_CHANNELS = {
  openApk: "apkrunner:open-apk",
  getApkInfo: "apkrunner:get-apk-info",
  startApp: "apkrunner:start-app",
  stopApp: "apkrunner:stop-app",
  getStatus: "apkrunner:get-status",
  pollEvents: "apkrunner:poll-events"
} as const;

export type IpcChannel = (typeof IPC_CHANNELS)[keyof typeof IPC_CHANNELS];
