# Tooling

This directory holds small development utilities.

Current tools:

- `check-renderer-imports.mjs` ensures renderer sources do not import Electron, Node built-ins, or filesystem APIs.
- `copy-native-artifact.mjs` copies the Cargo-built napi library to `apps/desktop/native/apkrunner_napi.node` for Electron development.

Future tooling can live here when it is cross-package or root-level. Package-specific scripts should stay beside the package that owns them.
