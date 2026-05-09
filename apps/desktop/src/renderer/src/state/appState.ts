import { createStore, produce } from "solid-js/store";
import type {
  ApkSummary,
  LogEntry,
  RuntimeEvent,
  RuntimeStatusValue,
  UnsupportedFeature
} from "../../../shared/protocol";

export type DevToolsTab = "Console" | "Info" | "Permissions" | "Surface" | "Unsupported";

export interface AppState {
  currentApk: ApkSummary | null;
  currentInstanceId: string | null;
  runtimeStatus: RuntimeStatusValue;
  logEntries: LogEntry[];
  runtimeEvents: RuntimeEvent[];
  unsupportedFeatures: UnsupportedFeature[];
  nativeAvailable: boolean;
  nativeLoadError: string | null;
  attemptedNativePaths: string[];
  currentError: string | null;
  activeTab: DevToolsTab;
  backendName: string;
}

const initialState: AppState = {
  currentApk: null,
  currentInstanceId: null,
  runtimeStatus: "Idle",
  logEntries: [],
  runtimeEvents: [],
  unsupportedFeatures: [],
  nativeAvailable: true,
  nativeLoadError: null,
  attemptedNativePaths: [],
  currentError: null,
  activeTab: "Console",
  backendName: "SkeletonRuntimeBackend"
};

export const [appState, setAppState] = createStore<AppState>(initialState);

function mergeUnsupported(apk: ApkSummary | null, runtimeEvents: RuntimeEvent[]): UnsupportedFeature[] {
  const features = [
    ...(apk?.unsupportedFeatures ?? []),
    ...runtimeEvents.flatMap((event) => (event.type === "UnsupportedFeature" ? [event.feature] : []))
  ];
  const byKey = new Map<string, UnsupportedFeature>();
  for (const feature of features) {
    byKey.set(`${feature.source}:${feature.feature}:${feature.detail}`, feature);
  }
  return [...byKey.values()];
}

function applyEvents(events: RuntimeEvent[]): void {
  if (events.length === 0) {
    return;
  }

  setAppState(
    produce((state) => {
      state.runtimeEvents.push(...events);
      state.logEntries.push(
        ...events.flatMap((event) =>
          event.type === "Log"
            ? [{ level: event.level, tag: event.tag, message: event.message, timestamp: event.timestamp }]
            : []
        )
      );
      for (const event of events) {
        if (event.type === "ApkLoaded") {
          state.currentApk = event.summary;
          state.runtimeStatus = "APK loaded";
        }
        if (event.type === "AppStarted") {
          state.runtimeStatus = "Running";
        }
        if (event.type === "AppStopped") {
          state.runtimeStatus = "Stopped";
        }
      }
      state.unsupportedFeatures = mergeUnsupported(state.currentApk, state.runtimeEvents);
    })
  );
}

function applyStatus(status: Awaited<ReturnType<typeof window.APKRunner.getStatus>>): void {
  if (!status.success) {
    setAppState({
      runtimeStatus: "Error",
      currentError: status.error.message
    });
    return;
  }

  setAppState({
    currentApk: status.data.currentApk,
    currentInstanceId: status.data.currentInstanceId,
    runtimeStatus: status.data.status,
    nativeAvailable: status.data.nativeAvailable,
    nativeLoadError: status.data.nativeLoadError,
    attemptedNativePaths: status.data.attemptedNativePaths,
    backendName: status.data.backendName,
    currentError: status.data.lastError,
    unsupportedFeatures: mergeUnsupported(status.data.currentApk, appState.runtimeEvents)
  });
}

export async function refreshStatus(): Promise<void> {
  applyStatus(await window.APKRunner.getStatus());
}

export async function openApk(): Promise<void> {
  const result = await window.APKRunner.openApk();
  if (!result.success) {
    setAppState({ runtimeStatus: "Error", currentError: result.error.message });
    await refreshStatus();
    return;
  }
  if (!result.data.canceled) {
    setAppState({
      currentApk: result.data.summary,
      currentInstanceId: result.data.instanceId,
      runtimeStatus: "APK loaded",
      currentError: null,
      unsupportedFeatures: mergeUnsupported(result.data.summary, appState.runtimeEvents)
    });
  }
  await pollEvents();
}

export async function startApp(): Promise<void> {
  const result = await window.APKRunner.startApp();
  if (!result.success) {
    setAppState({ runtimeStatus: "Error", currentError: result.error.message });
    return;
  }
  applyStatus(result);
  await pollEvents();
}

export async function stopApp(): Promise<void> {
  const result = await window.APKRunner.stopApp();
  if (!result.success) {
    setAppState({ runtimeStatus: "Error", currentError: result.error.message });
    return;
  }
  applyStatus(result);
  await pollEvents();
}

export async function pollEvents(): Promise<void> {
  const result = await window.APKRunner.pollEvents();
  if (!result.success) {
    setAppState({ currentError: result.error.message });
    return;
  }
  applyEvents(result.data);
}
