# APKRunner

APKRunner is a cross-platform Electron desktop APK Runner / Android Runtime Shell. It runs one Android APK at a time through a Rust runtime core exposed to Electron through a napi-rs bridge.

APKRunner does not implement Android in Rust. The runtime strategy is official-first: APKRunner owns and provisions an Android SDK/runtime bundle under app-controlled storage, then launches official Android runtime components from that bundle.

## Current Capabilities

- Electron desktop app with secure main, preload, and renderer layering.
- SolidJS renderer with a dark glass UI, APK picker, status toolbar, app surface placeholder, info panel, permissions panel, unsupported features panel, and logcat-like console.
- Rust APK loader that opens APK ZIP archives, parses binary AndroidManifest.xml, parses the primary DEX header, scans native libraries, assets, resources, permissions, multidex state, and compatibility risks.
- napi-rs bridge with JSON-compatible functions for runner creation, APK loading, app instance creation, start, stop, input dispatch, and event polling.
- Skeleton runtime backend that emits structured logs and placeholder frame events without executing Android code.
- Experimental `AospRuntimeBackend` that uses a managed Android Emulator, managed Platform Tools, managed AVD storage, `adb install`, activity launch, bounded logcat collection, PNG screencap capture, and `adb shell input`.
- Sandboxed virtual filesystem model for Android-style paths under a caller-provided host root.

## Managed Official Android Runtime

The AOSP backend is an encapsulated host for official Android tooling:

```text
Electron UI
  -> napi-rs bridge
  -> Rust APKRunner core
  -> APKRunner-managed Android SDK/runtime directory
  -> official Android Emulator + Platform Tools + system image + AVD
```

Normal users should not need Android Studio, `adb`, `ANDROID_HOME`, or PATH configuration. APKRunner resolves `sdkmanager`, `avdmanager`, `adb`, and `emulator` from its managed bundle. Debug overrides such as `APKRUNNER_ADB_PATH` are ignored unless `APKRUNNER_ALLOW_SYSTEM_ANDROID_TOOLS=1`.

The first managed backend uses the official Android Emulator because it is the practical cross-platform runtime base. APKRunner defaults to the lighter official ATD system image profile and launches the emulator headless with no audio, no boot animation, and software GPU rendering. First run may need to download official Android command-line tools and SDK packages unless a distribution ships or prepackages that bundle. Hardware virtualization, OS permissions, and upstream SDK license terms still apply.

The desktop app defaults to the managed AOSP backend. Use the Runtime panel to download or delete the managed runtime bundle under the app data directory. The download/install/delete work is implemented in Rust through the napi bridge; Electron only displays controls and progress.

Use the skeleton backend for parser-only development with:

```bash
APKRUNNER_BACKEND=skeleton npm run dev
```

## Install And Run

Prerequisites:

- Node.js 22.12.0 or newer
- npm 10 or newer
- Rust stable with Cargo
- A native build toolchain for napi-rs on your platform

Start the app:

```bash
npm install
npm run build:native
npm run dev
```

The desktop app still opens if the native addon has not been built. In that state it shows a native-addon unavailable panel with the attempted load paths and error details.

If Electron's binary download was interrupted, run:

```bash
npm run repair:deps
```

## Rust Addon Build

Build the Rust workspace:

```bash
npm run build:rust
```

Build the napi-rs crate:

```bash
npm run build:native
```

The script builds the Rust crate in release mode and copies the platform library to `apps/desktop/native/apkrunner_napi.node`. You can also set `APKRUNNER_NATIVE_PATH` to a custom built addon path.

## Tests

```bash
npm run test
```

Useful focused commands:

```bash
npm run test:rust
npm run test:ts
npm --workspace @apkrunner/desktop run typecheck
```

## Architecture

```text
Renderer (SolidJS, no Node APIs)
        |
        v
Preload contextBridge: window.APKRunner
        |
        v
Electron main IPC handlers
        |
        v
Safe native addon loader
        |
        v
napi-rs JSON bridge
        |
        v
Rust APKRunner core
        |
        +--> APK ZIP loader
        +--> Binary AXML parser
        +--> DEX parser
        +--> Permission manager
        +--> Virtual filesystem
        +--> Runtime backend trait
              |
              +--> SkeletonRuntimeBackend
              +--> AospRuntimeBackend
```

## Known Limitations

- APKRunner does not implement ART, Binder, Android Framework APIs, JNI, SurfaceFlinger, WebView, Flutter, React Native, Unity, or Google Play Services in Rust.
- Compatibility for those features comes only from the external official Android runtime selected by the backend.
- The first official runtime backend is an experimental managed Android Emulator backend.
- The default runtime profile is light, headless ATD, but it is still a VM because arbitrary APKs require Android system services.
- Cuttlefish is the long-term high-fidelity Linux/AOSP backend target, not the first cross-platform backend.
- Managed command-line tools download/extraction is represented in provisioning state; distributions should package the tools or complete the official download/license flow.

## Roadmap

See [docs/roadmap.md](docs/roadmap.md).
