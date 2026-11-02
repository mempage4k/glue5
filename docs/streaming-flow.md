[IA: Arena.ai | Agente/cliente: Arena.ai (Agent Mode — chat de streaming/servidor)]

# [SERV] Streaming flow: server to the game's file reads

This is a source map for the native GLUE 5 file-streaming protocol. It describes the current repository, not the separate HTTP/WebAssembly mirror service found in the supplied Windows audit.

## 1. Source folder and server preparation

- **Standalone server:** `crates/glue5-stream/src/main.rs` parses `glue5-serve` options, calls `server::prepare`, starts the TCP listener, and announces the port with `discovery::advertise`. On Windows, the supplied audit reports the executable is started with a mirror root containing `data/` and `b/<build>/shaders/`.
- **In-app desktop sharing:** `crates/glue5/src/app.rs` starts `sharing::Sharing` for the selected local game folder; `crates/glue5/src/sharing.rs` calls the same `server::prepare`, starts TCP (normally 47100, trying a few following ports if occupied), then advertises the chosen port.
- `crates/glue5-stream/src/catalog.rs` scans `data/` and the selected build's translated shaders. It excludes the three web-server helper files `manifest.json`, `bootset.json`, and `bootset_low.json` from the game catalog; hidden paths are skipped. Audio files have no special treatment.
- `crates/glue5-stream/src/server.rs` builds/loads the small-file and shader packs, includes boot-set metadata when present, and prepares a catalog and file handles for requests. RPFs and other large files are served as ordinary catalog entries.
- `crates/glue5-stream/src/protocol.rs` defines the native TCP protocol. It is not the HTTP static-file service (`serve_local.py`) described in the supplied Windows audit.

## 2. Discovery or manual address

- The in-app sharing and standalone server announce Bonjour/mDNS service `_glue5._tcp`: `crates/glue5-stream/src/discovery.rs`.
- On Android, the app holds the Wi-Fi multicast lock while using LAN discovery: `crates/glue5/src/lan.rs` and the Android activity implementation. The lock concerns local-network discovery; it does not provide Internet reachability.
- The Servers page invokes `App::find_servers` in `crates/glue5/src/app.rs`: it searches briefly, then checks found addresses against `glue5_runtime::GAME_BUILD` via `glue5_stream::check`.
- A user can enter a server address instead. The `Source::Server` / server setting path in `crates/glue5/src/app.rs` starts a client for that address. A typed address can be reachable over a routed/private network or VPN; discovery itself is LAN mDNS, and the native protocol has no encryption or access control (see `docs/design.md`).
- Server selection is not file-source priority. It selects the endpoint from which this stream session obtains the catalog and its files.

## 3. Greeting, catalog, and cache synchronization

- `crates/glue5-stream/src/client.rs` connects, reads the server greeting (`HELLO`), and rejects incompatible protocol/build metadata. The build compatibility check is in `check`/`unusable` near the end of that module.
- On first sync, the client receives the catalog, boot set and any required packs, then opens the local copy with `Cache::open`.
- Android cache location: `paths::cache_dir` in `crates/glue5/src/paths.rs` uses the app's external-files directory plus `cache`; the partial stream copy is `cache/stream/<GAME_BUILD>` (`paths::stream_copy`). It mirrors game-relative paths and has a `.glue5-stream` partial-copy marker; it is not a user-selected full game folder.
- `crates/glue5-stream/src/cache.rs` preserves a cached file's blocks only when catalog path, size, mtime, bitmap length, and on-disk file length still match. If a path disappears from the new catalog its old cached file is removed; if metadata changes, its old blocks are discarded and downloaded again. This is entry-by-entry validation, not wholesale deletion solely because the catalog hash changed.
- During an already-running partial stream, `client.rs` compares the server catalog hash with the synchronized catalog. A changed catalog fails that session with a restart-app message rather than silently switching the game's file map mid-run. A complete local download is treated specially and can keep playing its own catalog.
- `crates/glue5/src/app.rs` moves a partial copy between the cache and download location when the user's whole-game-download setting changes, and deletes an obsolete partial cache copy when appropriate. Whole-game downloads are not overwritten by a different server catalog.

## 4. File request, arrival, and engine read

1. `crates/glue5-runtime/src/stream.rs` connects the game's web-build HTTP-filesystem reads to GLUE 5. The app supplies the engine manifest from the synchronized catalog and hooks engine reads.
2. `Client::read` / request scheduling in `crates/glue5-stream/src/client.rs` answers a read from valid local cache blocks immediately. Missing blocks become `READ` requests; engine-waiting reads outrank announced prefetch, read-ahead, boot-set prefetch, and whole-game fill. The client can promote a pending request when the engine begins waiting.
3. `crates/glue5-stream/src/server.rs` queues requests by priority and transmits chunks; the client verifies received chunks and writes them to the sparse cache. Requests are retried after reconnection as needed.
4. The engine's waiting read resumes once its requested blocks are present. Over time the client also performs read-ahead for sequentially played files. The normal stream is demand-driven; it does not proactively download every audio archive simply because the archive is available on the server.

`Download the whole game` is a separate low-priority fill operation (`Client::set_fill` in `client.rs`, enabled by app settings in `app.rs`). It requests all catalog files; it is not a selective character-voice download.

## 5. Scope and open issues

- No reproduced open discovery/import bug has been established by this audit. A gap in Linear tracking is not evidence that the code has no bugs.
- The supplied Windows audit reports a separate static HTTP mirror server as well as `glue5-serve`. Only the latter speaks the GLUE native streaming protocol. Its Windows binary's source revision was not verified.
- The current README says the sole official target is the Samsung Galaxy S21 Plus on Android 11+, while `docs/streaming.md` and `docs/design.md` still describe iOS, tvOS, desktop, NAS, and cross-platform behavior. The implementation remains in the repository, but that historical documentation does not change the current support statement.
- Proposed Android-only user-doc wording (not yet applied pending product-owner approval): “On the supported Galaxy S21 Plus, choose **Stream from a server** or **Settings › Game › Game files › Server** to play from a computer running GLUE 5 sharing or `glue5-serve`. The phone discovers servers on the local network; you can also enter a reachable server address. The game runs on the phone; files are fetched as needed and kept in its local stream cache. **Download the whole game** is a separate optional download.” Keep the historical platform details in engineering notes unless the owner approves a broader documentation rewrite.

## Related source and docs

- `crates/glue5-stream/src/{discovery,client,cache,catalog,pack,protocol,server}.rs`
- `crates/glue5/src/{app,sharing,lan,paths}.rs`
- `crates/glue5-runtime/src/stream.rs`
- `docs/streaming.md`, `docs/design.md#streaming-the-games-files`
- `docs/windows-stream-audio-audit.md` (supplied Windows audit snapshot; not independently verified here)
