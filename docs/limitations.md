# Current Limitations

APKRunner v1 is a scaffold and inspector, not a full runtime.

Not supported:

- Real DEX execution
- ART
- Binder
- Android Framework
- JNI / native `.so` execution
- Flutter
- React Native
- Unity
- WebView
- SurfaceFlinger
- Google Play Services
- Android view hierarchy rendering
- Input dispatch to real Android activities
- APK code access to host filesystem outside a sandbox

Unsupported features are reported as structured data and displayed in the renderer. They should not panic, silently disappear, or be represented as successful execution.
