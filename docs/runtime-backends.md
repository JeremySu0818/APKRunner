# Runtime Backends

Runtime execution is behind the `RuntimeBackend` trait in `apkrunner-core`.

Every backend must provide:

- Backend name.
- App instance creation.
- App start.
- App stop.
- Runtime event polling.

## SkeletonRuntimeBackend

`SkeletonRuntimeBackend` is the only implemented backend in this scaffold. It validates lifecycle state and emits structured events, but it does not execute Android bytecode, JNI, framework APIs, or native libraries.

On start it emits:

- `APKRunner: Starting <package>`
- `APKRunner: Launcher Activity = <activity>`
- `APKRunner: Using SkeletonRuntimeBackend`
- `APKRunner: Runtime execution is not implemented yet`
- `APKRunner: Future backends: DexVmRuntimeBackend, AospRuntimeBackend, VmRuntimeBackend`

On stop it emits:

- `APKRunner: Stopped <package>`

## Future Backends

The codebase names future backend families so integration points are clear:

- `DexVmRuntimeBackend`
- `AospRuntimeBackend`
- `VmRuntimeBackend`
- `ArmTranslationRuntimeBackend`

Future backends should keep the existing JSON event protocol stable and add capability through the backend trait rather than through Electron-specific code.
