export const IPC_CHANNELS = {
  openApk: "apkrunner:open-apk",
  getApkInfo: "apkrunner:get-apk-info",
  startApp: "apkrunner:start-app",
  stopApp: "apkrunner:stop-app",
  dispatchInput: "apkrunner:dispatch-input",
  getRuntimeBundleStatus: "apkrunner:get-runtime-bundle-status",
  startRuntimeDownload: "apkrunner:start-runtime-download",
  startRuntimeDelete: "apkrunner:start-runtime-delete",
  getRuntimeOperationStatus: "apkrunner:get-runtime-operation-status",
  getStatus: "apkrunner:get-status",
  pollEvents: "apkrunner:poll-events"
} as const;

export type IpcChannel = (typeof IPC_CHANNELS)[keyof typeof IPC_CHANNELS];
