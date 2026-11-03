# Optional game-content package manifest (protocol v1 extension)

This document defines the first server/client contract for logical content packages. It is deliberately separate from `packs.text` and `packs.shaders`: those remain compressed transport packs, while package members are ordinary, ordered catalog entries and are not downloaded unless a future client feature explicitly selects them.

## Discovery and backward compatibility

A server may provide a UTF-8 JSON sidecar named `packages.json` in its game root (beside `data/` and `b/`). It is not placed under `data/`, so it is not added to the engine manifest or catalog IDs. `glue5-serve` validates the sidecar against its selected build and catalog during `prepare`.

When valid, the existing `HELLO` JSON gains `server_id`, `packages_hash` (SHA-256 of the exact manifest bytes), and `blobs.packages` (the uncompressed sidecar byte length); the regular blob request names `packages` to receive the manifest. No protocol number or existing HELLO fields change. A pre-existing client ignores unknown HELLO members and continues its normal catalog/stream flow. A server without the sidecar, or with an invalid/oversized sidecar, starts without package discovery and continues normal streaming.

`server_id` is an identifier asserted by the sidecar and echoed in HELLO, not cryptographic server authentication. Protocol v1 has no authentication or transport encryption. Do not use this field as a security boundary.

## Schema version 1

```json
{
  "schema_version": 1,
  "server_id": "stable-manager-assigned-id",
  "build": "the-exact-build-id",
  "catalog_hash": "the-server-catalog-hash",
  "protocol_min": 1,
  "protocol_max": 1,
  "packages": [
    {
      "id": "voice-en",
      "name": "Character dialogue (English)",
      "description": "Optional package description",
      "kind": "dialogue",
      "version": "1",
      "target_build": "the-exact-build-id",
      "catalog_hash": "the-server-catalog-hash",
      "protocol_min": 1,
      "protocol_max": 1,
      "dependencies": [],
      "files": [
        {
          "path": "data/x64/audio/sfx/example.rpf",
          "size": 1234,
          "sha256": "64 hexadecimal digits",
          "catalog_id": 17,
          "dependencies": []
        }
      ],
      "total_size": 1234,
      "optional": true,
      "available": true,
      "locale": "en",
      "created_at": "2026-11-02T00:00:00Z"
    }
  ]
}
```

`kind` is an extensible string, not a closed enum. Package IDs use lower-case ASCII letters, digits, `.`, `_` and `-`. Paths are canonical relative `/` paths under `data/`; they cannot be absolute, contain `.`/`..`, empty segments, backslashes or drive prefixes. Every file must already exist in the catalog with exactly the same path, size and numeric catalog ID. The ID is checked, never used to reorder catalog entries. Package, build, catalog and protocol constraints are validated before the client exposes the manifest.

The catalog hash is only a catalog identity (paths, sizes and timestamps); it is not a content checksum. Each package file therefore has its own SHA-256. The server validates the digest's syntax and the catalog metadata, but does not hash tens of gigabytes during startup. A package installer must verify the actual downloaded bytes before marking a file installed. Manifest dependencies refer to package IDs; duplicate, missing, self-referential and cyclic dependencies are rejected. Optional file-level `dependencies` are canonical relative paths and are reserved for future dependency handling.

## Current implementation boundary

The Rust streaming server/client support the optional blob and schema validation. The client API can resolve dependency order, download one selected package through catalog range reads without toggling the whole-game fill flag, pause/cancel between ranges, resume from the existing block cache, verify the actual SHA-256, persist installation receipts and pin verified files against cache eviction. `Client::package_is_installed` reports a matching receipt while all package files remain complete. The Servers page lists the optional packages from the validated manifest, shows which are installed or unavailable, and runs one transfer at a time with pause, resume and cancel. This remains separate from downloading the whole game; a server without a valid manifest simply offers no optional content.

The external Go Asset Manager is not part of this repository and must generate/publish `packages.json` using this schema before the Windows-managed server advertises packages. Until then, a server without that file is fully usable for the normal streaming flow but reports no optional package manifest. Package transfer support does not classify RPF contents or establish a validated dialogue inventory; no game files are included in this repository.

A package manifest can list only assets that the selected build's catalog already serves. This interface does not classify RPF contents or certify that a package contains only dialogue; package inventories and their dependencies must be independently reviewed before publication. Never commit or attach proprietary game assets or extracted proprietary inventories to this repository or Linear.
