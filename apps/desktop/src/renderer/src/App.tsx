import { For, Show, createEffect, createMemo, onCleanup, onMount } from "solid-js";
import type { JSX } from "solid-js";
import {
  AlertTriangle,
  Boxes,
  Bug,
  Circle,
  Download,
  FileArchive,
  FolderOpen,
  Info,
  Monitor,
  Play,
  Shield,
  Square,
  Terminal,
  Trash2
} from "lucide-solid";
import { GlassProvider } from "./components/glass/GlassProvider";
import { GlassButton, GlassPanel, GlassToolbar } from "./components/glass/GlassPrimitives";
import {
  appState,
  dispatchInput,
  openApk,
  pollEvents,
  pollRuntimeOperation,
  refreshRuntimeBundleStatus,
  refreshStatus,
  setAppState,
  startApp,
  startRuntimeDelete,
  startRuntimeDownload,
  stopApp,
  type DevToolsTab
} from "./state/appState";
import type { CompatibilityLevel, LogEntry, PermissionRecord, UnsupportedFeature } from "../../shared/protocol";

const tabs: Array<{ id: DevToolsTab; label: string }> = [
  { id: "Console", label: "Console" },
  { id: "Info", label: "Info" },
  { id: "Permissions", label: "Permissions" },
  { id: "Surface", label: "Surface" },
  { id: "Unsupported", label: "Unsupported" }
];

function boolText(value: boolean): string {
  return value ? "Yes" : "No";
}

function optionalText(value: string | number | null | undefined): string {
  return value === null || value === undefined || value === "" ? "Unknown" : String(value);
}

function compatibilityClass(level: CompatibilityLevel): string {
  return `compat compat-${level.toLowerCase()}`;
}

function Toolbar(): JSX.Element {
  const currentName = createMemo(() => appState.currentApk?.fileName ?? "No APK loaded");
  return (
    <GlassToolbar class="runtime-toolbar">
      <div class="brand-block">
        <Boxes size={22} aria-hidden="true" />
        <div>
          <strong>APKRunner</strong>
          <span>Android runtime shell</span>
        </div>
      </div>
      <div class="toolbar-meta">
        <span class="meta-label">APK</span>
        <span class="meta-value">{currentName()}</span>
      </div>
      <div class="toolbar-meta">
        <span class="meta-label">Backend</span>
        <span class="meta-value">{appState.backendName}</span>
      </div>
      <div class="status-pill">
        <Circle size={10} class={`status-dot status-${appState.runtimeStatus.toLowerCase().replace(" ", "-")}`} />
        {appState.runtimeStatus}
      </div>
    </GlassToolbar>
  );
}

function Sidebar(): JSX.Element {
  const canStart = createMemo(() => Boolean(appState.currentApk) && appState.runtimeStatus !== "Running");
  const canStop = createMemo(() => appState.runtimeStatus === "Running");
  const runtimeBusy = createMemo(() => appState.runtimeOperation?.state === "installing" || appState.runtimeOperation?.state === "deleting");
  const runtimeDisplayState = createMemo(() => {
    if (runtimeBusy() || appState.runtimeOperation?.state === "error") {
      return appState.runtimeOperation?.state;
    }
    return appState.runtimeBundle?.state ?? "unknown";
  });
  const runtimeDisplayMessage = createMemo(() => appState.runtimeOperation?.message ?? appState.runtimeBundle?.message ?? "Runtime status unavailable");
  const runtimeDisplayError = createMemo(() => appState.runtimeOperation?.error ?? appState.runtimeBundle?.error);

  return (
    <GlassPanel class="sidebar">
      <div class="sidebar-actions">
        <GlassButton type="button" onClick={() => void openApk()}>
          <FolderOpen size={18} aria-hidden="true" />
          Open APK
        </GlassButton>
        <GlassButton type="button" disabled={!canStart()} onClick={() => void startApp()}>
          <Play size={18} aria-hidden="true" />
          Start App
        </GlassButton>
        <GlassButton type="button" disabled={!canStop()} onClick={() => void stopApp()}>
          <Square size={18} aria-hidden="true" />
          Stop App
        </GlassButton>
      </div>
      <div class="runtime-bundle-box">
        <div class="runtime-bundle-header">
          <span>Runtime</span>
          <strong>{runtimeDisplayState()}</strong>
        </div>
        <div class="runtime-bundle-message">
          {runtimeDisplayError() ?? runtimeDisplayMessage()}
        </div>
        <Show when={appState.runtimeOperation?.progress !== null && appState.runtimeOperation?.progress !== undefined}>
          <div class="runtime-progress">
            <span style={{ width: `${Math.round((appState.runtimeOperation?.progress ?? 0) * 100)}%` }} />
          </div>
        </Show>
        <div class="runtime-actions">
          <GlassButton type="button" disabled={runtimeBusy()} onClick={() => void startRuntimeDownload()}>
            <Download size={16} aria-hidden="true" />
            Download
          </GlassButton>
          <GlassButton type="button" disabled={runtimeBusy()} onClick={() => void startRuntimeDelete()}>
            <Trash2 size={16} aria-hidden="true" />
            Delete
          </GlassButton>
        </div>
      </div>
      <div class="sidebar-status">
        <span>Status</span>
        <strong>{appState.runtimeStatus}</strong>
      </div>
      <Show when={appState.currentError}>
        <div class="inline-error">
          <AlertTriangle size={16} aria-hidden="true" />
          <span>{appState.currentError}</span>
        </div>
      </Show>
    </GlassPanel>
  );
}

function NativeUnavailablePanel(): JSX.Element {
  return (
    <div class="native-error">
      <div class="native-error-title">
        <AlertTriangle size={20} aria-hidden="true" />
        Native addon unavailable
      </div>
      <p>{appState.nativeLoadError ?? "The Rust napi-rs addon could not be loaded."}</p>
      <Show when={appState.attemptedNativePaths.length > 0}>
        <ul>
          <For each={appState.attemptedNativePaths}>{(path) => <li>{path}</li>}</For>
        </ul>
      </Show>
    </div>
  );
}

function SurfacePanel(): JSX.Element {
  let surfaceRef: HTMLDivElement | undefined;

  function handleSurfaceTap(event: MouseEvent): void {
    if (!appState.latestFrame || appState.latestFrame.frameFormat !== "Png" || !surfaceRef) {
      return;
    }
    const bounds = surfaceRef.getBoundingClientRect();
    const x = Math.max(0, Math.min(1, (event.clientX - bounds.left) / bounds.width));
    const y = Math.max(0, Math.min(1, (event.clientY - bounds.top) / bounds.height));
    void dispatchInput({
      type: "tap",
      x: Math.round(x * appState.latestFrame.surfaceSize.width),
      y: Math.round(y * appState.latestFrame.surfaceSize.height)
    });
  }

  return (
    <GlassPanel class="surface-panel">
      <Show when={appState.nativeAvailable} fallback={<NativeUnavailablePanel />}>
        <Show
          when={appState.currentApk}
          fallback={<div class="empty-surface">Open an APK to inspect manifest, DEX, permissions, and runtime compatibility.</div>}
        >
          <div
            class={`surface-stage ${appState.runtimeStatus === "Running" ? "surface-running" : ""}`}
            ref={surfaceRef}
            onClick={handleSurfaceTap}
          >
            <Show when={appState.latestFrame?.frameFormat === "Png"} fallback={<Monitor size={46} aria-hidden="true" />}>
              <img
                class="surface-frame"
                src={`data:image/png;base64,${appState.latestFrame?.payloadBase64 ?? ""}`}
                alt=""
              />
            </Show>
            <div class="surface-message">
              <strong>{appState.currentApk?.packageName}</strong>
              <span>Runtime backend: {appState.backendName}</span>
            </div>
          </div>
        </Show>
      </Show>
    </GlassPanel>
  );
}

function FieldRow(props: { label: string; value: string | number | null | undefined }): JSX.Element {
  return (
    <div class="field-row">
      <span>{props.label}</span>
      <strong>{optionalText(props.value)}</strong>
    </div>
  );
}

function ApkInfoPanel(): JSX.Element {
  return (
    <GlassPanel class="info-panel">
      <div class="panel-title">
        <Info size={18} aria-hidden="true" />
        APK Info
      </div>
      <Show when={appState.currentApk} fallback={<div class="panel-empty">No APK loaded</div>}>
        {(apk) => (
          <>
            <div class={compatibilityClass(apk().compatibilityLevel)}>{apk().compatibilityLevel}</div>
            <FieldRow label="Package" value={apk().packageName} />
            <FieldRow label="Version name" value={apk().versionName} />
            <FieldRow label="Version code" value={apk().versionCode} />
            <FieldRow label="Minimum SDK" value={apk().minSdk} />
            <FieldRow label="Target SDK" value={apk().targetSdk} />
            <FieldRow label="Launcher" value={apk().launcherActivity} />
            <FieldRow label="DEX classes" value={apk().dexClassCount} />
            <FieldRow label="Multidex" value={boolText(apk().multidex)} />
            <FieldRow label="resources.arsc" value={boolText(apk().hasResourcesArsc)} />
            <FieldRow label="Native libraries" value={boolText(apk().hasNativeLibraries)} />
            <FieldRow label="Native ABIs" value={apk().nativeAbis.join(", ") || "None"} />
          </>
        )}
      </Show>
    </GlassPanel>
  );
}

function PermissionItem(props: { permission: PermissionRecord }): JSX.Element {
  return (
    <li class={`permission-item ${props.permission.dangerous ? "permission-danger" : ""}`}>
      <div>
        <strong>{props.permission.name}</strong>
        <span>{props.permission.description}</span>
      </div>
      <em>{props.permission.state}</em>
    </li>
  );
}

function PermissionPanel(): JSX.Element {
  return (
    <GlassPanel class="permission-panel">
      <div class="panel-title">
        <Shield size={18} aria-hidden="true" />
        Permissions
      </div>
      <Show when={(appState.currentApk?.requestedPermissions.length ?? 0) > 0} fallback={<div class="panel-empty">No permissions requested</div>}>
        <ul class="permission-list">
          <For each={appState.currentApk?.requestedPermissions ?? []}>{(permission) => <PermissionItem permission={permission} />}</For>
        </ul>
      </Show>
    </GlassPanel>
  );
}

function LogLine(props: { entry: LogEntry }): JSX.Element {
  return (
    <div class={`log-line log-${props.entry.level.toLowerCase()}`}>
      <span>[{props.entry.timestamp}]</span>
      <strong>{props.entry.tag}:</strong>
      <span>{props.entry.message}</span>
    </div>
  );
}

function LogcatConsole(): JSX.Element {
  let consoleRef: HTMLDivElement | undefined;

  createEffect(() => {
    appState.logEntries.length;
    queueMicrotask(() => {
      if (consoleRef) {
        consoleRef.scrollTop = consoleRef.scrollHeight;
      }
    });
  });

  return (
    <GlassPanel class="console-panel">
      <div class="panel-title">
        <Terminal size={18} aria-hidden="true" />
        Logcat
      </div>
      <div class="console-output" ref={consoleRef}>
        <Show when={appState.logEntries.length > 0} fallback={<div class="panel-empty">Runtime logs will appear here</div>}>
          <For each={appState.logEntries}>{(entry) => <LogLine entry={entry} />}</For>
        </Show>
      </div>
    </GlassPanel>
  );
}

function UnsupportedItem(props: { feature: UnsupportedFeature }): JSX.Element {
  return (
    <li class={`unsupported-item unsupported-${props.feature.severity.toLowerCase()}`}>
      <AlertTriangle size={16} aria-hidden="true" />
      <div>
        <strong>{props.feature.feature}</strong>
        <span>{props.feature.detail}</span>
      </div>
    </li>
  );
}

function UnsupportedFeaturesPanel(): JSX.Element {
  return (
    <GlassPanel class="unsupported-panel">
      <div class="panel-title">
        <Bug size={18} aria-hidden="true" />
        Unsupported
      </div>
      <Show when={appState.unsupportedFeatures.length > 0} fallback={<div class="panel-empty">No unsupported features detected</div>}>
        <ul class="unsupported-list">
          <For each={appState.unsupportedFeatures}>{(feature) => <UnsupportedItem feature={feature} />}</For>
        </ul>
      </Show>
    </GlassPanel>
  );
}

function DevToolsTabs(): JSX.Element {
  return (
    <GlassPanel class="devtools-panel">
      <div class="tab-bar">
        <For each={tabs}>
          {(tab) => (
            <button
              type="button"
              class={appState.activeTab === tab.id ? "active" : ""}
              onClick={() => setAppState({ activeTab: tab.id })}
            >
              {tab.label}
            </button>
          )}
        </For>
      </div>
      <div class="tab-body">
        <Show when={appState.activeTab === "Console"}>
          <LogcatConsole />
        </Show>
        <Show when={appState.activeTab === "Info"}>
          <ApkInfoPanel />
        </Show>
        <Show when={appState.activeTab === "Permissions"}>
          <PermissionPanel />
        </Show>
        <Show when={appState.activeTab === "Surface"}>
          <SurfacePanel />
        </Show>
        <Show when={appState.activeTab === "Unsupported"}>
          <UnsupportedFeaturesPanel />
        </Show>
      </div>
    </GlassPanel>
  );
}

export function App(): JSX.Element {
  onMount(() => {
    void refreshStatus();
    void refreshRuntimeBundleStatus();
    const timer = window.setInterval(() => {
      void pollEvents();
      void pollRuntimeOperation();
    }, 900);
    onCleanup(() => window.clearInterval(timer));
  });

  return (
    <GlassProvider>
      <div class="app-shell">
        <Toolbar />
        <div class="workspace-grid">
          <Sidebar />
          <SurfacePanel />
          <div class="right-column">
            <ApkInfoPanel />
            <PermissionPanel />
          </div>
        </div>
        <div class="bottom-grid">
          <DevToolsTabs />
          <UnsupportedFeaturesPanel />
        </div>
      </div>
    </GlassProvider>
  );
}
