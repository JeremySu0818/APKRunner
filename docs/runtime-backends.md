# Runtime Backends

Runtime execution is behind the `RuntimeBackend` trait in `apkrunner-core`.

Every backend must provide:

- Backend name.
- App instance creation.
- App start.
- App stop.
- Input dispatch.
- Runtime event polling.

## SkeletonRuntimeBackend

`SkeletonRuntimeBackend` validates lifecycle state and emits structured events, but it does not execute Android bytecode, JNI, framework APIs, or native libraries.

On start it emits:

- `APKRunner: Starting <package>`
- `APKRunner: Launcher Activity = <activity>`
- `APKRunner: Using SkeletonRuntimeBackend`
- `APKRunner: Runtime execution is not implemented yet`
- `APKRunner: Future backends: DexVmRuntimeBackend, AospRuntimeBackend, VmRuntimeBackend`

On stop it emits:

- `APKRunner: Stopped <package>`

## AospRuntimeBackend

`AospRuntimeBackend` is an experimental managed official Android Emulator backend. It is no longer a placeholder.

It manages runtime paths under APKRunner-controlled storage:

- `bundleRoot/sdk`
- `bundleRoot/avd`
- `bundleRoot/emulator-home`

It resolves tools from that bundle:

- `cmdline-tools/latest/bin/sdkmanager`
- `cmdline-tools/latest/bin/avdmanager`
- `platform-tools/adb`
- `emulator/emulator`

It uses `sdkmanager` to install required SDK packages, `avdmanager` to create an APKRunner-owned AVD, and managed `adb` for APK install, launch, bounded logcat, PNG screencap, force-stop, and input dispatch. It does not use the user's `adb`, emulator, Android Studio install, `ANDROID_HOME`, or PATH by default.

The default package plan is the lightest official emulator profile APKRunner can use while still running Android APKs:

- `cmdline-tools;latest`
- `platform-tools`
- `emulator`
- `platforms;android-35`
- `system-images;android-35;google_atd;<host abi>`

ATD means Automated Test Device. Google describes ATD emulator images as optimized to reduce CPU and memory usage by removing components that normally do not affect app tests. APKRunner also starts the emulator headless by default with no audio, no boot animation, and software GPU rendering. This is still a VM; it is the lightest official managed runtime profile, not a Rust implementation of Android.

If command-line tools are missing, provisioning returns `NeedsCommandLineToolsDownload`. APKRunner does not hardcode unofficial download URLs; distributions should package command-line tools or implement the official download/license flow.

For development or distribution packaging, `prepackagedCmdlineToolsRoot` or `APKRUNNER_PREPACKAGED_CMDLINE_TOOLS_ROOT` can point at an already extracted official command-line tools directory. APKRunner copies that directory into `sdk/cmdline-tools/latest` before running managed `sdkmanager`.

The Electron GUI exposes runtime download/delete/progress controls. Those controls call Rust napi functions; the Rust installer owns official command-line-tools download, SDK package installation, AVD creation, manifest writing, and bundle deletion.

Frame capture currently uses `adb exec-out screencap -p` and emits `FrameFormat::Png`. Emulator gRPC frame streaming is a future optimization once protobuf generation and token/JWT auth handling are implemented.

## Future Backends

The codebase names future backend families so integration points are clear:

- `DexVmRuntimeBackend`
- `VmRuntimeBackend`
- `ArmTranslationRuntimeBackend`

Future backends should keep the existing JSON event protocol stable and add capability through the backend trait rather than through Electron-specific code.

Cuttlefish is the long-term high-fidelity Linux/AOSP backend candidate, especially for platform development workflows. Android Emulator remains the first cross-platform official runtime base.
