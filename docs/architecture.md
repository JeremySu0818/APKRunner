# Architecture

APKRunner is split into layers so desktop UI concerns never leak into the Rust runtime core.

## Layering

```text
SolidJS renderer
  -> preload contextBridge API
  -> Electron main IPC handlers
  -> native addon loader
  -> napi-rs bridge
  -> apkrunner-core Rust crate
```

The renderer imports only renderer-safe TypeScript protocol types and calls `window.APKRunner`. It does not import `electron`, Node built-ins, Rust internals, or the native addon.

The preload exposes exactly:

- `openApk`
- `getApkInfo`
- `startApp`
- `stopApp`
- `getStatus`
- `pollEvents`

All IPC handlers return `{ success: true, data }` or `{ success: false, error }`.

## IPC Channels

- `apkrunner:open-apk`
- `apkrunner:get-apk-info`
- `apkrunner:start-app`
- `apkrunner:stop-app`
- `apkrunner:get-status`
- `apkrunner:poll-events`

## APK Load Flow

1. Renderer asks preload to open an APK.
2. Main process opens the native file dialog.
3. Main process calls the napi bridge.
4. Rust opens the APK as a ZIP archive.
5. Rust parses `AndroidManifest.xml` as Android binary XML.
6. Rust parses `classes.dex` for header and class summaries.
7. Rust scans resources, native libraries, assets, permissions, multidex state, and unsupported features.
8. Rust returns a JSON-compatible APK summary.

## App Start Flow

1. Renderer calls `startApp`.
2. Main process validates state and calls the napi bridge.
3. Rust starts the app instance through `RuntimeBackend`.
4. `SkeletonRuntimeBackend` emits log events, an unsupported-runtime feature event, an app-started event, and a placeholder frame event.
5. Renderer polls events and displays logs, surface state, and unsupported features.
