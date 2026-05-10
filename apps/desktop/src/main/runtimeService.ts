import { dialog } from "electron";
import { basename } from "node:path";
import type {
  ApkSummary,
  InputEvent,
  OpenApkResult,
  RuntimeBundleStatus,
  RunnerStatus,
  RuntimeEvent,
  RuntimeOperationStatus,
  RuntimeStatusValue
} from "../shared/protocol";
import { getAppDataPaths } from "./appData";
import { loadNativeAddon, type NativeAddon } from "./native";

interface RunnerState {
  runnerId: string | null;
  backendName: string;
  currentApk: ApkSummary | null;
  currentInstanceId: string | null;
  status: RuntimeStatusValue;
  lastError: string | null;
}

export class RuntimeService {
  private state: RunnerState = {
    runnerId: null,
    backendName: "SkeletonRuntimeBackend",
    currentApk: null,
    currentInstanceId: null,
    status: "Idle",
    lastError: null
  };

  private nativeAddon(): NativeAddon {
    const native = loadNativeAddon();
    if (!native.available || !native.addon) {
      this.state.status = "Error";
      this.state.lastError = native.error ?? "Native addon is unavailable.";
      throw new Error(this.state.lastError);
    }
    return native.addon;
  }

  private ensureRunner(): string {
    if (this.state.runnerId) {
      return this.state.runnerId;
    }

    const addon = this.nativeAddon();
    const paths = getAppDataPaths();
    const backendKind = process.env.APKRUNNER_BACKEND?.toLowerCase() === "skeleton" ? "Skeleton" : "Aosp";
    const runnerConfig: Record<string, unknown> = {
      backendKind,
      sandboxRoot: paths.sandboxRoot
    };

    if (backendKind === "Aosp") {
      const runtimeBundle: Record<string, unknown> = {
        bundleRoot: paths.androidRuntimeRoot,
        launchEmulator: true,
        allowSystemToolOverrides: process.env.APKRUNNER_ALLOW_SYSTEM_ANDROID_TOOLS === "1"
      };
      if (process.env.APKRUNNER_ADB_PATH) {
        runtimeBundle.debugAdbPath = process.env.APKRUNNER_ADB_PATH;
      }
      if (process.env.APKRUNNER_EMULATOR_PATH) {
        runtimeBundle.debugEmulatorPath = process.env.APKRUNNER_EMULATOR_PATH;
      }
      if (process.env.APKRUNNER_SDKMANAGER_PATH) {
        runtimeBundle.debugSdkmanagerPath = process.env.APKRUNNER_SDKMANAGER_PATH;
      }
      if (process.env.APKRUNNER_AVDMANAGER_PATH) {
        runtimeBundle.debugAvdmanagerPath = process.env.APKRUNNER_AVDMANAGER_PATH;
      }
      runnerConfig.runtimeBundle = runtimeBundle;
    }

    const created = addon.createRunner(runnerConfig);
    this.state.runnerId = created.runnerId;
    this.state.backendName = created.backendName;
    this.state.status = "Idle";
    this.state.lastError = null;
    return created.runnerId;
  }

  status(): RunnerStatus {
    const native = loadNativeAddon();
    return {
      nativeAvailable: native.available,
      nativeLoadError: native.error,
      attemptedNativePaths: native.attemptedPaths,
      runnerId: this.state.runnerId,
      currentApk: this.state.currentApk,
      currentInstanceId: this.state.currentInstanceId,
      status: this.state.status,
      backendName: this.state.backendName,
      lastError: this.state.lastError
    };
  }

  apkInfo(): ApkSummary | null {
    return this.state.currentApk;
  }

  async openApk(): Promise<OpenApkResult> {
    const selection = await dialog.showOpenDialog({
      title: "Open Android APK",
      properties: ["openFile"],
      filters: [{ name: "Android APK", extensions: ["apk"] }]
    });

    if (selection.canceled || selection.filePaths.length === 0) {
      return {
        canceled: true,
        summary: this.state.currentApk,
        instanceId: this.state.currentInstanceId
      };
    }

    const apkPath = selection.filePaths[0];
    const addon = this.nativeAddon();
    const runnerId = this.ensureRunner();
    const loaded = addon.loadApk(runnerId, apkPath);
    const summary: ApkSummary = {
      ...loaded,
      fileName: loaded.fileName || basename(apkPath)
    };
    const instance = addon.createAppInstance(runnerId, {
      packageName: summary.packageName
    });

    this.state.currentApk = summary;
    this.state.currentInstanceId = instance.instanceId;
    this.state.status = "APK loaded";
    this.state.lastError = null;

    return {
      canceled: false,
      summary,
      instanceId: instance.instanceId
    };
  }

  startApp(): RunnerStatus {
    const addon = this.nativeAddon();
    const runnerId = this.ensureRunner();
    if (!this.state.currentInstanceId || !this.state.currentApk) {
      this.state.status = "Error";
      this.state.lastError = "No APK is loaded.";
      throw new Error(this.state.lastError);
    }
    if (this.state.backendName === "AospRuntimeBackend" && !this.runtimeBundleStatus().installed) {
      this.state.status = "Error";
      this.state.lastError = "Download the managed Android runtime before starting an APK.";
      throw new Error(this.state.lastError);
    }

    addon.startApp(runnerId, this.state.currentInstanceId);
    this.state.status = "Running";
    this.state.lastError = null;
    return this.status();
  }

  stopApp(): RunnerStatus {
    const addon = this.nativeAddon();
    const runnerId = this.ensureRunner();
    if (!this.state.currentInstanceId) {
      this.state.status = "Error";
      this.state.lastError = "No app instance is loaded.";
      throw new Error(this.state.lastError);
    }

    addon.stopApp(runnerId, this.state.currentInstanceId);
    this.state.status = "Stopped";
    this.state.lastError = null;
    return this.status();
  }

  dispatchInput(input: InputEvent): RunnerStatus {
    const addon = this.nativeAddon();
    const runnerId = this.ensureRunner();
    if (!this.state.currentInstanceId) {
      this.state.status = "Error";
      this.state.lastError = "No app instance is loaded.";
      throw new Error(this.state.lastError);
    }

    addon.dispatchInput(runnerId, this.state.currentInstanceId, input);
    this.state.lastError = null;
    return this.status();
  }

  runtimeBundleStatus(): RuntimeBundleStatus {
    const addon = this.nativeAddon();
    const paths = getAppDataPaths();
    return addon.getRuntimeBundleStatus({ bundleRoot: paths.androidRuntimeRoot });
  }

  startRuntimeDownload(): RuntimeOperationStatus {
    const addon = this.nativeAddon();
    const paths = getAppDataPaths();
    return addon.startRuntimeDownload({ bundleRoot: paths.androidRuntimeRoot });
  }

  startRuntimeDelete(): RuntimeOperationStatus {
    const addon = this.nativeAddon();
    const paths = getAppDataPaths();
    return addon.startRuntimeDelete({ bundleRoot: paths.androidRuntimeRoot });
  }

  runtimeOperationStatus(operationId: string): RuntimeOperationStatus {
    const addon = this.nativeAddon();
    return addon.getRuntimeOperationStatus(operationId);
  }

  pollEvents(): RuntimeEvent[] {
    const native = loadNativeAddon();
    if (!native.available || !native.addon || !this.state.runnerId) {
      return [];
    }
    const events = native.addon.pollEvents(this.state.runnerId);
    for (const event of events) {
      if (event.type === "AppStarted") {
        this.state.status = "Running";
      }
      if (event.type === "AppStopped") {
        this.state.status = "Stopped";
      }
      if (event.type === "ApkLoaded") {
        this.state.currentApk = event.summary;
        this.state.status = "APK loaded";
      }
    }
    return events;
  }
}
