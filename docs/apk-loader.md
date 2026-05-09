# APK Loader

APK files are ZIP archives. APKRunner opens them strictly for inspection and never executes contained code.

## ZIP Inventory

The loader scans:

- `AndroidManifest.xml`
- `classes.dex` and additional `classesN.dex` files
- `resources.arsc`
- `lib/<abi>/*.so`
- `assets/*`

## Android Binary XML

Android manifests are commonly stored as chunked binary XML. APKRunner validates the file chunk, reads the string pool, parses start/end element chunks, and builds a document tree with elements, attributes, and children.

The parser supports manifest metadata needed by v1:

- `manifest`
- `uses-permission`
- `uses-sdk`
- `application`
- `activity`
- `intent-filter`
- `action`
- `category`

Invalid magic, unexpected EOF, malformed chunks, and unsupported chunk types return structured errors.

## DEX Header Parsing

The DEX parser reads:

- Magic
- Adler-32 checksum
- SHA-1 signature
- File size
- Header size
- Endian tag
- String IDs
- Type IDs
- Method IDs
- Class definitions

The parser produces class summaries where enough DEX metadata is available. It does not execute DEX bytecode.

## Compatibility Report

Compatibility is based on deterministic inspection:

- `libflutter.so` -> Red, Flutter unsupported.
- `libreactnativejni.so` -> Red, React Native unsupported.
- `libunity.so` or Unity player class -> Red, Unity unsupported.
- Any native `.so` -> Yellow, native execution unsupported.
- Multidex -> Yellow.
- Google Play Services references -> Yellow.
- No native libraries, single DEX, and no modeled unsupported features -> Green.
