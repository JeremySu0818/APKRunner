# Current Limitations

APKRunner is an APK inspector plus an experimental host for official Android runtime components. It is not a Rust reimplementation of Android.

Not supported:

- ART implemented in Rust
- Binder implemented in Rust
- Android Framework APIs implemented in Rust
- JNI / native `.so` execution implemented in Rust
- SurfaceFlinger implemented in Rust
- WebView implemented in Rust
- Flutter, React Native, Unity, or Google Play Services implemented in Rust
- Android view hierarchy rendering
- APK code access to host filesystem outside a sandbox

Unsupported features are reported as structured data and displayed in the renderer. They should not panic, silently disappear, or be represented as successful execution.

The experimental `AospRuntimeBackend` can execute APKs only through official external Android runtime components that APKRunner manages under app-controlled directories. That backend currently uses Android Emulator, Platform Tools, SDK packages, an APKRunner-created AVD, managed `adb install`, managed `adb shell input`, bounded logcat, and PNG screencap.

APKRunner's default AOSP profile uses official ATD emulator images and headless emulator flags to reduce CPU, memory, audio, windowing, and boot-animation overhead. It remains a VM. Running arbitrary APKs without a VM or a real Android device would require replacing ART, Binder, Framework services, package manager behavior, SurfaceFlinger/display plumbing, and app lifecycle semantics.

Remaining limitations:

- Command-line tools acquisition is not hardcoded; provisioning reports `NeedsCommandLineToolsDownload` unless command-line tools are packaged or explicitly provided.
- Hardware virtualization and OS emulator permissions may be required.
- Google APIs / Play services compatibility depends on the selected official system image and upstream licensing.
- Cuttlefish is planned as the higher-fidelity Linux/AOSP backend, not the default cross-platform backend.
