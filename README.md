# APKRunner

APKRunner is a cross-platform Electron desktop APK Runner / Android Runtime Shell. It runs one Android APK at a time through a Rust runtime core exposed to Electron through a napi-rs bridge.

This first version is an engineering scaffold and APK inspector with a skeleton runtime backend. It is intentionally honest about its limits.

**This version does NOT execute APK bytecode.**

**This version is NOT a complete Android emulator.**

**Full execution requires future DexVmRuntimeBackend, AospRuntimeBackend, or VmRuntimeBackend.**

## Current Capabilities

- Electron desktop app with secure main, preload, and renderer layering.
- SolidJS renderer with a dark glass UI, APK picker, status toolbar, app surface placeholder, info panel, permissions panel, unsupported features panel, and logcat-like console.
- Rust APK loader that opens APK ZIP archives, parses binary AndroidManifest.xml, parses the primary DEX header, scans native libraries, assets, resources, permissions, multidex state, and compatibility risks.
- napi-rs bridge with JSON-compatible functions for runner creation, APK loading, app instance creation, start, stop, and event polling.
- Skeleton runtime backend that emits structured logs and placeholder frame events without executing Android code.
- Sandboxed virtual filesystem model for Android-style paths under a caller-provided host root.

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
              v
        SkeletonRuntimeBackend
```

## Known Limitations

- No real DEX execution.
- No ART runtime.
- No Binder.
- No Android Framework implementation.
- No JNI or native `.so` execution.
- Flutter, React Native, Unity, WebView, SurfaceFlinger, and Google Play Services are not supported.
- The app surface is a deterministic placeholder frame, not a real Android view hierarchy.

## Roadmap

See [docs/roadmap.md](docs/roadmap.md).
