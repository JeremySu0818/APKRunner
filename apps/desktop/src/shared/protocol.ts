export type CompatibilityLevel = "Green" | "Yellow" | "Red" | "Unknown";

export type RuntimeStatusValue = "Idle" | "APK loaded" | "Running" | "Stopped" | "Error";

export type LogLevel = "DEBUG" | "INFO" | "WARN" | "ERROR";

export type PermissionState = "Granted" | "Denied" | "AskEveryTime" | "Unsupported";

export interface UnsupportedFeature {
  feature: string;
  detail: string;
  severity: "Info" | "Warning" | "Error";
  source: "APK" | "Runtime";
}

export interface PermissionRecord {
  name: string;
  state: PermissionState;
  dangerous: boolean;
  description: string;
}

export interface ApkSummary {
  fileName: string;
  packageName: string;
  versionName: string | null;
  versionCode: number | null;
  minSdk: number | null;
  targetSdk: number | null;
  launcherActivity: string | null;
  requestedPermissions: PermissionRecord[];
  dexFiles: string[];
  dexClassCount: number;
  multidex: boolean;
  hasResourcesArsc: boolean;
  hasNativeLibraries: boolean;
  nativeAbis: string[];
  nativeLibraries: NativeLibrarySummary[];
  assets: string[];
  unsupportedFeatures: UnsupportedFeature[];
  compatibilityLevel: CompatibilityLevel;
}

export interface NativeLibrarySummary {
  path: string;
  abi: string;
  name: string;
}

export interface SurfaceSize {
  width: number;
  height: number;
}

export type FrameFormat = "Rgba8888" | "PlaceholderText" | "Png";

export interface LogEntry {
  level: LogLevel;
  tag: string;
  message: string;
  timestamp: string;
}

export type RuntimeEvent =
  | ({ type: "Log" } & LogEntry)
  | { type: "ApkLoaded"; summary: ApkSummary }
  | { type: "PermissionRequest"; permission: PermissionRecord }
  | { type: "AppStarted"; packageName: string; instanceId: string }
  | { type: "AppStopped"; packageName: string; instanceId: string }
  | { type: "UnsupportedFeature"; feature: UnsupportedFeature }
  | {
      type: "FrameReady";
      surfaceSize: SurfaceSize;
      frameFormat: FrameFormat;
      payloadBase64: string;
      metadata: string;
      timestamp: string;
    };

export interface RunnerStatus {
  nativeAvailable: boolean;
  nativeLoadError: string | null;
  attemptedNativePaths: string[];
  runnerId: string | null;
  currentApk: ApkSummary | null;
  currentInstanceId: string | null;
  status: RuntimeStatusValue;
  backendName: string;
  lastError: string | null;
}

export interface OpenApkResult {
  canceled: boolean;
  summary: ApkSummary | null;
  instanceId: string | null;
}

export type InputEvent =
  | { type: "tap"; x: number; y: number }
  | { type: "key"; keyCode: number }
  | { type: "text"; text: string };

export type RuntimeInstallState = "notInstalled" | "installed" | "installing" | "deleting" | "error";

export interface RuntimeBundleStatus {
  state: RuntimeInstallState;
  bundleRoot: string;
  sdkRoot: string;
  avdHome: string;
  avdName: string;
  manifestPath: string;
  installed: boolean;
  phase: string;
  message: string;
  progress: number | null;
  error: string | null;
}

export interface RuntimeOperationStatus {
  operationId: string;
  state: RuntimeInstallState;
  phase: string;
  message: string;
  progress: number | null;
  error: string | null;
  bundleStatus: RuntimeBundleStatus | null;
}

export interface IpcError {
  code: string;
  message: string;
  details?: unknown;
}

export type IpcResult<T> =
  | { success: true; data: T }
  | { success: false; error: IpcError };

export interface APKRunnerPreloadApi {
  openApk(): Promise<IpcResult<OpenApkResult>>;
  getApkInfo(): Promise<IpcResult<ApkSummary | null>>;
  startApp(): Promise<IpcResult<RunnerStatus>>;
  stopApp(): Promise<IpcResult<RunnerStatus>>;
  dispatchInput(input: InputEvent): Promise<IpcResult<RunnerStatus>>;
  getRuntimeBundleStatus(): Promise<IpcResult<RuntimeBundleStatus>>;
  startRuntimeDownload(): Promise<IpcResult<RuntimeOperationStatus>>;
  startRuntimeDelete(): Promise<IpcResult<RuntimeOperationStatus>>;
  getRuntimeOperationStatus(operationId: string): Promise<IpcResult<RuntimeOperationStatus>>;
  getStatus(): Promise<IpcResult<RunnerStatus>>;
  pollEvents(): Promise<IpcResult<RuntimeEvent[]>>;
}

declare global {
  interface Window {
    APKRunner: APKRunnerPreloadApi;
  }
}

export {};
