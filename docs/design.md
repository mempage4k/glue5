# Design decisions

This document covers the architecture, platform constraints and tradeoffs
behind Glue 5.

## One application for every platform

`crates/glue5` is the application: a library with one entry point
(`glue5::main`) and a thin binary for desktop. Each platform only adds the
way the operating system starts a process:

| Platform | Started by | Packaging |
|---|---|---|
| macOS, Linux, Windows | the `glue5` binary | `cargo build` |
| iOS | `platform/ios/main.c` calls `glue5_main`, which hands the process to SDL (`SDL_RunApp`) | an Xcode project generated from `platform/ios/project.yml` by xcodegen links the crate as a static library (`mise run ios`) |
| tvOS (Apple TV) | the same `main.c` and `glue5_main` | likewise, from `platform/tvos/project.yml`, the crate built for `aarch64-apple-tvos` (`mise run tvos`) |
| Android (next) | SDL's Java activity loads the library and calls `SDL_main` | Gradle project with SDL's activity, the library built with the NDK |

Platform-specific Rust code uses `cfg`s for details such as default folders
and window flags. The app shares its UI and event handling across platforms.
The generated Xcode project replaces the earlier `glue5-ios` crate and
cargo-mobile2 setup; there is no Swift UI or Xcode project to maintain by
hand.

## Compiling the game module

`game.wasm` uses 64-bit memory, threads and SIMD. wasm2c translates it to C,
then the platform's compiler builds native code ahead of time. Firefox's
RLBox sandboxes use the same approach.

We considered WebAssembly runtimes such as Wasmtime, Wasmer and WAMR.
Their usual ahead-of-time paths map compiled code as executable at run
time, which conflicts with iOS restrictions. Interpreters are too slow for
this game, and the static compilation modes are less established. These
runtimes also check the bounds of each access to 64-bit memory. Glue 5
treats the game as trusted code and disables those checks through wasm2c's
`WASM_RT_NONCONFORMING_*` modes.

wasm2c leaves several pieces to the host: a C function for each import,
trap handling around calls into the module, Emscripten's setjmp/longjmp
wrappers (`invoke_*`) and an instance per thread. Trap handling uses setjmp
and must stay in C.

A longjmp crosses host frames: a trap or an Emscripten longjmp under a hook
that called back into the module, or an import that unwinds on purpose
(`emscripten_exit_with_live_runtime`). On Unix it only restores registers.
MSVC's longjmp instead unwinds like an exception, running the handlers of
those frames, and Rust aborts in an `extern "C"` function it unwinds. On
Windows the jump buffers lose their frame before the jump, which makes the
CRT's longjmp behave as everywhere else (`glue5.h`, `patches/wabt/0006`).

On Windows the C is compiled with clang-cl, Clang with MSVC's command line,
headers and ABI, and linked by link.exe with the Rust side. cl.exe
truncates identifiers past 2047 characters, which the engine's mangled
names reach, and its C11 atomics, which wasm2c's threads need, are
experimental.

The `glue5-game` build script generates this glue by reading the module
with wasmparser. Imports use a function-pointer table filled by Rust at
startup, so the generated C does not depend on Rust symbols or static
library link order. Exports return traps as values, and hooks replace game
functions by name. The Rust crate exposes the FFI bindings.

The game is x86 code, where a core's stores reach the others in order;
on ARM they need not, and wasm's plain accesses to shared memory promise
no order either. Where the game publishes data with plain stores (the
physics' collision pairs: an entry, then the list's count, read by worker
threads meanwhile), the build script makes those accesses atomic in the
generated C (`build/ordering.rs`, checked against the exact text); a
worker on a Pixel 8a otherwise read a stale pair now and then, and the
engine's bounds checks trapped.

Two more fixes are made the same way. Some resources of this game data are
placed in more pages than their page map has room for; the extra entries
overwrote what follows the map (the Vinewood Hills fences then stopped the
game with "Invalid fixup"). Such a resource gets a page map in the heap
(`build/page_map.rs`; `-nodefrag` keeps resources from being moved). And
the engine's messages that print sizes with MSVC's `%Iu` use `%zu`
(`build/formats.rs`): the game's libc stopped at them.

SDL3 handles windows, input, gamepads, audio and dialogs. Other dependencies
include wgpu, naga, egui, parking_lot_core for wasm futexes, bcdec_rs for BC
decoding, jiff for the system's time zone, png and serde_json. Project code
implements the Emscripten system interface, translates the engine's command
ring to wgpu, adapts the shaders and supplies touch controls.

On Windows, mimalloc is the allocator: the renderer and wgpu allocate and
free many blocks every frame, and the system heap is slower at it (with it
the game's benchmark ran 4-7 % slower).

We patch wasm2c, wgpu, wgpu-core, wgpu-hal and sp-fidelity. The patch files live in
`patches/` and `mise run setup` applies them to pinned sources. Cargo
cannot apply patch files directly to a dependency.

The wasm2c patches add wait/notify and SIMD on shared memory, trusted modes,
a fixed memory base, a hook where traps occur, indirect calls that pass
the caller's instance (so every thread's instance shares one function
table, 2.9 MB a thread otherwise) and, on Windows, longjmp without
unwinding and inline floating-point classification. The wgpu-core
patches clear usage scopes and drop command buffers' trackers by their
members, reuse those trackers, stop pipelines from retaining shader modules, let a device
poll wait for the GPU without holding the device's resource lock, and
record a pass's barriers at the end of the command buffer before it
instead of in one of their own (one raw command buffer per pass, not two),
reducing CPU work during pass encoding, memory use and stalls between
threads. The wgpu-hal patches make occlusion queries count samples on Metal
and Vulkan, as D3D11's do (the game measures the sun's visibility with
them), and reset them on Metal before reuse (a slot no draw wrote kept an
older query's count: the lens flare showed through walls), make the depth
bias a pass's state, and keep Vulkan command pools to the command buffers their last use needed (on Mali each holds ~0.4 MB of
host memory), and make a Metal command buffer only when a command needs it
(wgpu-core's barrier buffer before each pass is empty on Metal, and Metal
charges per command buffer: 30 % of the process's CPU time went to Metal's
own threads on an iPhone 16 Pro, 23 % after), and let a command buffer be
suspended and resumed (for wgpu-core's one per pass), and make a Vulkan
command pool give its memory back every 16th reset (~110 MB of Mali driver
memory on a Pixel 8a). With one command
buffer per pass, the game's benchmark ran 5 % faster on a Pixel 8a, 15 %
in the scenes where memory runs short, and its encoding took 12-14 ms a
frame instead of 17-23; on an iPhone 16 Pro (with the Metal command buffers
made when needed) it ran 5 % faster, 63.1 and 60.8 fps against 60.0 and
57.8, submitting 28 % shorter. The sp-fidelity patch passes FSR 2's
accumulate pass whole variables where it passed a function pointers into a
struct, which SPIR-V (Vulkan) does not take and naga's SPIR-V writer panics
on. We would like to submit these changes upstream.

## Game files are never in the app

The game's data is 20 GB (`data/`), plus this build's translated shaders
(`b/<build>/shaders/`, 155 MB). Neither goes in the application package:

- it is the user's copy of the game, not ours to distribute;
- 20 GB is far over what app stores accept, and every app update would
  carry it again;
- the shaders belong to the game build (the build checks `game.wasm`'s
  hash), so they live and move with the data.

The application looks for a game directory (`data/` + `b/<build>/shaders/`),
in this order: the command line (desktop), `GLUE5_GAME_DIR`, the last
location (`glue5.json`), the platform's default places, then (desktop) a
folder picker. Default places:

| Platform | Default | How the files get there |
|---|---|---|
| desktop | `game` next to the executable or in the repository | copy, or pick any folder |
| iOS | the app's `Documents/game` | Finder (device > Files) or the Files app; `UIFileSharingEnabled` and `LSSupportsOpeningDocumentsInPlace` make the folder visible |
| Android | the app's external files directory, `Android/data/<app>/files/game` | USB file transfer, or Find files: a folder anywhere on the device's storage, read where it is |

**Find files on Android** is the folder picker, not a rescan: a phone has no
`adb` and Android 11 and later hide `Android/data` from file managers, so the
app's own folder is often not one the player can put 20 GB into. The picker
(`ACTION_OPEN_DOCUMENT_TREE`) answers with a tree whose local id is a path
(`primary:Download/game`), which the activity hands to the app as that path;
nothing is copied, and the engine reads the game where it is. The grant a
tree comes with does not help here: it reaches files through the document
provider (a binder call an open, and no path the VFS could `stat`), and
copying through it would need the room twice over on a phone that already
holds the files. Reading by path instead takes all-files access
(`MANAGE_EXTERNAL_STORAGE`), which the activity asks for when Find files is
first used and not before: a game copied into the app's own folder, the way
`mise run android:game` puts it there, needs no permission at all. Without
the access, Find files explains that it is needed and lets the player retry
once granted. The app now requires Android 11 (API 30) so the all-files access
flow is available. A folder chosen one level above the game's (Downloads, an SD
card's root) is resolved down to it. The choice goes back as a file in the imports folder,
which survives the activity being recreated while the picker was up.

Reads of a folder elsewhere on the storage go through Android's
shared-storage layer (FUSE, since Android 11), of which the app's own
external directory sees the least; what that costs the engine's reads is not
measured, so the copy in `Android/data/<app>/files/game` stays the place to
recommend where the room for it exists. Find files is for getting going
without a computer, without `adb` (and a phone that hides `Android/data`),
and without twice the room that copying in would take.

Or the files come from another device: see the next section.

## Streaming the game's files

Settings › Game › Game files › Server plays the game from a computer that
has it: the app on a computer sharing its folder (Settings › Game › Share
on the network), or `glue5-serve` (`mise run serve`, any desktop system, a
NAS). A phone then needs no copy of its own, and an Apple TV, whose
storage is only a cache the system empties when it needs room, fetches
again what it lost. Both modes live side by side: with a folder, nothing
below runs.

**The engine already streams.** The browser build reads `/game` through
an HTTP device of its own (`fiDeviceLocal` over `wasm_httpfs_*`): a
manifest of the files, one read at a time per thread, and a ring in which
the streamer announces the reads it has queued (`pgStreamer::Read`) and
those it expects (`strStreamingLoader`). glue5 hands it the manifest
(`wasm_httpfs_manifest_js`) and hooks its read (`wasm_httpfs_pread`,
`glue5-runtime/src/stream.rs`): a read the device's copy holds is a
positioned read from it in the calling thread, as fast as a local folder;
a missing one asks the server and waits for its blocks only. The
announcements become prefetch requests, and so does what follows a file
played over time (music and radio, read in pieces 20 ms or more apart;
not small pieces read back to back: read ahead, they fetched 520 MB in a
cold run, 340 MB of it never read). Streaming in the VFS instead would
get no announcements (they need the HTTP device) and one round trip per
read of the streamer's single reader.

**What it has to carry** (the benchmark's reads, traced in local mode):
3.0 GB of distinct data in 189 s, 2.3 GB of it in the first 30 s (start
and first world), then ~5 MB/s, 12 MB/s at most over 10 s, ~320 MB in
5 s at a scene cut. The engine hardly reads anything twice (3.17 GB read
for 3.03 GB distinct): within a session a cache gains nothing, across
sessions everything. The archives (`.rpf`, 19.4 of 19.5 GB) are already
compressed or encrypted: zstd takes 1 % off them. The small files are
text: 3,700 of them (385 MB: shader effects, XML, metadata) make 8.6 MB
as one zstd stream (level 19, alike files side by side, a 32 MB window),
the 6,600 translated shaders 1.6 MB.

**The protocol** (`glue5-stream`, `protocol.rs`) is one TCP connection with
32-byte message headers, not HTTP: the server sends the next 64 KB chunk of
the most urgent request first (an engine read waiting > a queued read
announced > a guess or read-ahead > the boot set > a full download), keeps
little unsent in the kernel (`TCP_NOTSENT_LOWAT`, 128 KB), and moves a
request to a higher priority when the engine comes to wait for it.
Requests are one chunk each, so that such a move takes one chunk: behind
32 MB of prefetch on an 8 MB/s link, a read the engine waits for arrives
in 10-14 ms (134 ms with 1 MB requests; seconds with the browser build's
parallel HTTP requests, which share the link evenly). A cold disk on the
server reads ahead of the queue (`F_RDADVISE`, `posix_fadvise`). Each chunk
carries a CRC-32 (TCP's checksum is weak, and a bad block would stay in the
copy): a damaged one is dropped and asked again, as is one whose header
(which has none) puts it outside its request. Requests are pipelined
without limit, so the link, not the round trip, sets the pace: through a
link of 100 Mbit/s and 40 ms (simulated), the first start kept it 96 % busy
and had the world on screen after 3 min 20 s (2.3 GB), the benchmark then
running as long as from a folder; over the local loopback, after 36 s.
TCP, not UDP: every byte must arrive, and a reliable, congestion-controlled
transport over UDP is QUIC, a heavy dependency, worth it only if lossy
links turn out to need it. The client drops a connection with requests in
flight and no data for 20 s (counted from the request, after a quiet
spell), or which takes no request for 20 s (a write timeout: a stalled
server holds up neither the app nor stopping); the server lets go of a
client gone without a word after a minute (TCP keepalive: no read
timeout, as a client may ask nothing for long).

**Reads announced late.** Traced through a proxy that delays each way by
20 ms and sends 12.5 MB/s behind a 256 KB queue, a cold benchmark run
waits for ~11,000 engine reads. Until the world shows, the link sets the
pace. In the fly-through some read waits a third to half of the time, and
a fifth of the time it is one of ~1,000 reads announced too late: blocks
nobody had asked for (~40 a run), or asked for less than 10 ms before,
mostly by guesses the engine announces as it makes the read. Such a read
waits ~60 ms (the round trip and the queue), one announced in time ~20 ms.
(The client's log once counted 6,000 of them, from a read's first block
alone, often here already from the read before.) A table learned on the
server across sessions (the blocks that come after a request), prefetched
by the next session, would hide few: which reads come late depends on when
the engine announces them, not on the blocks, and nine in ten of one run's
late reads had been asked for in time in another run. Learned from one
run, such a table foresaw a quarter to a third of the next run's late
reads on the very same path, bringing 76-94 MB for 14 MB of them (from two
runs, two fifths, with 155 MB); in play the paths differ, and only a
device's first session would gain (its copy keeps the rest). Not done. Nor
does the engine know of them much earlier: it makes its guesses at the
start of `strStreamingLoader::LoadRequestedObjects`, for objects it has
been asked to load (at most 32 a call, a cap rarely reached), and loads
them in the same call. A probe of `strStreamingInfoManager::RequestObject`
found that the game had asked for the late ones less than 10 ms before the
read in half the cases, and less than 50 ms before in nine in ten:
announcing them at the request would have saved a fifth of their waiting
in the fly-through (8.6 s of 41), not worth a hook on the engine's request
path.

**Frames while streaming.** Every frame of cold benchmark runs, laid
against every wait of the client: once the world shows, no read waits on
the engine's main or render thread, only on the streamer's reader (a late
read is an object that comes late, not a slow frame: 5-6 s of waiting in
the fly-through over the loopback, 130 s through the simulated 100 Mbit/s
link). The client's threads take 2-3 % of a core. The index saves are the
one thing of the client's that shows in the frames (see the copy below).
Slow frames came and went far more with the Mac's load from other work
than with streaming: a run from a folder had more of them than streamed
runs. (The simulated link, a Python proxy, also slows down under load:
8.3 to 10.4 MB/s from run to run; a start needs 2.8 GB before the world
either way.)

**Before the game:** the catalog (every file, size and time, asked with
the boot set's ranges), then the packs it does not have, ~10 MB, asked
together (two round trips), which bring every small file and the
shaders whole: the pipelines compile from them as from a folder (Play's
preparation is unchanged). Then, at the lowest priority but one, the boot
set (the whale5 build's `bootset.json`: the ~550 MB of archive headers and
start-up data every start reads), while the start page shows and the
shaders compile; a later start finds it there. If the game starts before
it is all here (a quick Play over the Internet: 550 MB take 44 s at 100
Mbit/s), the engine reads it in an order of its own, not the boot set's
(file after file, by path): when it waits for a part still in the boot
set's queue, the 8 MB of the boot set that follow come next, as
read-ahead. Through the simulated link, three cold starts each way: the
engine's waits for the boot set while the world loads went from 35 to 26
s, and the world showed after 192 s instead of 195. Little more is to be
had in order: the link is full from the start to the world, which needs
~2.3 GB (the benchmark reads 2.2 GB before it shows, from a folder).

**The copy** (`cache.rs`) mirrors the folder's layout, sparse, with a
bitmap of the 4 KB blocks each file holds (640 KB for the whole game). Its
index is saved after the data it describes is on disk (`fsync` per file,
side by side, then `F_FULLFSYNC` on the index on Apple systems: a full
flush per file made the text pack take 14 s to save): every 10 s while
data comes, or once 256 MB came, and at once when it stops. A save slows
the game's frames a little while it lasts (1.2 to 1.8 times as many
frames over 50 ms fall within saves as within the same spans a second
away; an I/O priority of background or `F_BARRIERFSYNC` made no
difference that could be told from run to run), so fewer is better:
saving every two seconds, a cold run through the simulated link saved 203
times, now 47. At
start, a block counts only if its file is still there with its full
length: whatever the system deleted meanwhile (iOS and tvOS caches) is
fetched again when needed. When the volume falls under 1 GB free, files
read longest ago are emptied, 256 MB beyond that at a time: data files of
earlier runs first, then this run's least recently used, the boot set's
files after them (read first, they were the least recently used), the
packs' files last (an iPhone with a gigabyte free gave away a gigabyte at
a time, the start's own files and the packs among them). An index without
their blocks is saved before their data is freed (a crash in between
would otherwise trust zeros). Emptying that raises no free space
(something else fills the volume) stops until the volume has room again:
it would give away the whole copy for nothing. A copy whose packs are
complete starts without the server (missing blocks wait for it); a lost
connection is retried for ever (at once after one that served, then at
growing intervals) and the requests in flight asked again, in the order
they were asked. Stopping a client (another server chosen, the copy
deleted) returns at once: it finishes in the background, its index saved
last, and the next client, like moving or deleting a copy, waits for it.
APFS, when it writes a file out, may fill the holes between its written
parts with zeros (whether it does depends on its state, and changed within
an hour on the same Mac: 5.7 GB taken for 3.7 GB of blocks); after each
save, the holes of the files just synced are punched again
(`F_PUNCHHOLE`): 3.74 GB for 3.67. Each hole holds off the file's writes
while it is punched, not its reads. Other file systems never fill holes,
and are left alone. At most 4,096 files stay open (the limit is 10,240 on
Apple systems, often 1,024 on Linux); the others are opened for each use.

**The whole game** (Download the whole game, on the Servers page) comes at the
lowest priority, 32 MB on its way at a time, with what the game leaves of
the link; a complete copy loses its marker and is a game folder like a
copied one, which plays without the server and which the local mode finds.
It goes where a copied game would (`Documents/game`, the external files'
`game`, the user directory's `game` on desktop), not in the cache folder,
and never over a game folder of the user's own. Complete, it stays there
whatever the setting says later, used as it is and never emptied for room
(moved back into the cache, an iPhone's system emptied 17 of its 21 GB),
nor changed by a server whose files differ (played with its own catalog);
only a partial copy moves when the setting changes, and one left in the
cache beside the whole game is deleted. tvOS has no such option:
it keeps nothing but caches.

**Where:** the app's cache folder (`paths::cache_dir`): `Library/Caches`
on iOS and tvOS (purged by the system when away, which suits it),
`~/Library/Caches` on macOS, `XDG_CACHE_HOME` on Linux, `%LOCALAPPDATA%`
on Windows (files marked sparse, which they are not by default there),
the external files folder on Android. The copy carries a marker, so that
it is never taken for a game folder.

**Finding a server** (`discovery.rs`): Bonjour, `_glue5._tcp`. A server
announces itself on every system with a responder of its own (port 5353,
shared with the system's: mDNSResponder, Avahi, Windows', whose APIs differ
on each): on each network with that network's address, answering by
multicast, or by unicast to simple resolvers and unicast-response queries,
saying that its host has no IPv6 address (NSEC: a resolver does not wait
for one), and saying goodbye when it stops. Its host is
`glue5-<computer>.local`, its name the computer's (with the port, if not
47100: two servers on one computer); no probing for unique names. A Mac's
own browser and an iPhone find it, as Avahi does on Linux (in a
container); not tried on Windows. Apple systems browse through the
system's responder (no multicast entitlement on iOS and tvOS; the service
is in `NSBonjourServices`); elsewhere the app asks once, by mDNS from an
ordinary port, and responders answer by unicast (no Android multicast
lock). The Servers page
searches every five seconds while it is open and asks each server found
for its greeting (`check`): whether it serves this game build, how long
connecting took, and what its name stands for, so that a server typed as
its IP shows under the name it was found by. A Mac's `.local` name stands
for its IPv6 link-local address too, where a server listening on IPv4
refuses: the client tries each address, IPv4 first.

Limits: there is no encryption and no access control (on the Internet,
a private network such as Tailscale gives both); the server must have the
app's game build; files changed on the server while the game runs need a
restart. A partial copy without its server plays until the first block it
lacks, and waits there (a radio station starts at a random place: the
benchmark waited at its loading screen). Over the game a small round sign
says so (a turning ring: waiting for the server; amber: the server does
not answer), in words for three seconds after it changes; it sits left of
the settings button or in the next free place, off the touch controls and
the overlay, and a tap opens the Servers page.
The reads the engine announces late (~1,000 in the benchmark) cost a
round trip each: on a slow link, objects that show late, not stalls.

## Shaders: known pipelines before the game

The 4919 translated WGSL shaders are adapted and compiled, with their render
pipelines, on background threads. Pipelines are what costs: each combines a
shader pair, vertex layout, blend/depth/raster states and target formats, so
they cannot all be enumerated, and the combinations appear during play.
Every pipeline the game asks for is recorded
(`<user dir>/cache/pipelines.jsonl`, with the vertex shader's input
signature, which places the layout's elements). A recipe keeps only what
its pipeline depends on (the state words it ignores are zeroed): engine
states that differ elsewhere share one pipeline, about one in eight of the
seed's.

Before the game starts, Play compiles the known pipelines on every core: the
app's seed (`crates/glue5-gpu/seed/pipelines.jsonl`, recipes merged from
playthroughs on several devices with `mise run shaders:seed`; hashes and
states only, no game data) and this device's recordings. The start page
shows how far it is; the renderer then takes the pipelines over. This is
the usual practice (Steam's Fossilize replay, Unreal's PSO cache): record,
ship the set, compile it before the world shows.

The driver's cache makes it cheap after the first time: Metal keeps
compiled shaders in the system cache (macOS and iOS); on Vulkan the app
saves wgpu's pipeline cache per adapter and driver in `cache/`. Measured
with the ~5,600-recipe seed: an iPhone 13 Pro compiles a pipeline it has not
seen in ~200 ms of CPU while the game runs, one in its cache in ~2 ms; it
prepares the whole seed in 80-100 s the first time, a Mac in 56 s, and
0.9 s after. Without this, the first run on the phone showed the world with
objects and lights missing for about 40 s after the loading screen (20,000
draws skipped in 10 s, the compile threads saturated), because the world is
not drawn behind the loading screen and its pipelines appear only when it
shows; with it, 564 draws were skipped in those 10 s, then a few dozen.

Recipes include the program variants the renderer picks for what is bound
(depth buffers read as textures, everywhere; 32-bit float textures read
unfiltered, on GPUs that cannot filter them, such as the A15 and most
Android GPUs). The light and foliage shaders use them, so they are recorded
and prepared too. Compute pipelines are recorded and prepared alike (the
shader, its variant, the formats of the storage textures bound); a dispatch
whose pipeline is not ready makes it then, as its results are needed.

Pipelines never recorded still compile in the background during play: a
draw whose pipeline is not ready is skipped for that frame instead of
waiting, so a new pipeline never stalls a frame; the cost is an object
missing for a frame or two. They are recorded for the next start.

The radar's antialiased pixel shader expects edge distances from a geometry
shader, which WebGPU cannot run. Its load-time adapter supplies full coverage
instead; the texture, depth and clip-plane calculations stay intact. This is
specific to the radar: missing outputs in other stage pairs still fail
validation. Triangle-edge antialiasing from the geometry stage is not emulated.

The minimap's HUD mask uses normalized vertices, a five-tap texture filter
and the engine's mask constants. It has a separate cached pixel program,
reflected bindings and pipeline recipe flag; its vertex variant converts
normalized coordinates to clip space. Pause-map exploration is identified
by its MIN alpha blend and keeps the original depth pixel program with the
world-space transform vertex shader. Neither correction changes game assets.
The same mask variant is selected when compiling recorded recipes ahead.

## Application UI and game menus

The engine supplies the PC front end, including loading screens, the pause
menu (Esc / Start), map, settings and phone.

The application uses [egui] for:

- a start page showing the game files, Play and Settings before the engine
  loads, with instructions if files are missing;
- a settings page (resolution, upscaling, VSync, frame rate, touch
  controls, low memory, game folder), opened from the start page or, in the
  game, with F1, the ⚙ button on touch screens or holding Back + Start on a
  controller;
- the touch controls.

All pages share the game's window. SDL's mobile backends restrict multiple
windows, while a second desktop window would add another surface and
separate focus and input routing. One home layout keeps the menu and status in two columns, with independent
scrolling. Text, controls and padding scale together on smaller screens. Settings use a centered, fixed floating window fitted to the safe
area, using almost its full height on compact screens. Scroll padding moves
with the content so items reach the window edges. Its tabs scroll with the options; each option shares the same rounded
row and right-aligned control. Home and pre-game settings share an embedded
image and gradient; in-game settings leave the game frame visible.

The UI is split into SDL input/overlay integration, pages, shared widgets
and a theme. Fonts and the home image are embedded, so appearance does not
depend on installed system fonts. Barlow SemiBold and Barlow Condensed Bold
are distributed with their SIL Open Font License in `crates/glue5/assets/ui`.
The widgets support controllers and keyboards: D-pad, left stick or arrows
move focus, A / Enter activates, and B / Esc goes back. Held directions
repeat.

The application creates the GPU device and surface (`glue5_gpu::Screen`)
before starting the engine. It uses them to draw the start page. Play
transfers both to the game's renderer, which then draws the overlay after
each game frame. The device is reused, with one owner of the surface at a
time.

The start page replaces the previous message boxes and allows changes to
startup settings, such as frame rate and low memory,
before the engine loads. It also lets users adjust settings after a failed
launch. Automatic restarts happen only when requested by the game itself
on desktop, using `--play`. The engine cannot start twice in one process:
its runtime is global and shutdown never completes. Returning to the start
page from the game is therefore unsupported; Quit ends the app.

The app draws the ⚙ button independently of the touch controls. It remains
visible at reduced opacity when a controller is in use or touch controls
are disabled. It consumes touches before they reach the virtual controller,
so opening settings leaves the physical controller in pad slot 0.

egui receives SDL events directly and renders at the window's resolution,
independently of the game's render scale. Native UI would require separate
SwiftUI and Android implementations. Using Scaleform would require hooks
into the game's menus, while iced or Slint would add their own event loops
and renderers.

Pages are laid out in egui's content rect: the window less the safe area
(notch, rounded corners, home indicator, from SDL) and, while a page is
open, less the on-screen keyboard, so a sheet ends above the keyboard and
a field being edited scrolls into sight inside it (`widgets::keep_in_view`).
SDL says when the keyboard shows, not where it is; `keyboard.rs` learns its
height from the platform: on iOS UIKit's keyboard notifications (its end
frame in the window); on Android 11 and later the IME inset, reported by
SDL's surface (`Glue5Surface`), with the window set not to pan. For the
game's text box, which the game draws where it does, Android pans the
window as before (the app switches the mode before the keyboard shows),
and Android 10, which has no IME inset, always pans. On iOS SDL still
shifts its view when the text area handed to it would be under the
keyboard, which it no longer is for the app's fields. What is drawn over
the game keeps to the safe area and off the touch controls.

Graphics quality remains in the game's menu to avoid conflicting settings
(see Known limits).

## Resolution, resizing and upscaling

The game's back buffer is the window's size in pixels multiplied by the
render scale (25-100% per axis). Dimensions are rounded to even numbers,
with a minimum of 320x240 and a maximum of 4K, or 1440p in low-memory mode.
The first start chooses a scale targeting roughly 1080 lines on desktop
and 720 on phones to limit GPU load on high-resolution screens. Existing
settings files are migrated from a fixed height to a scale. The UI shows
both the render and output dimensions.

Resizing affects three resources:

| What | Cost | When |
|---|---|---|
| The surface (swapchain) | `Surface::configure` waits for the GPU to be idle | at the next frame after the size changes |
| The engine's back buffer | a device reset recreates screen-sized render targets | once the size is stable |
| The MetalFX scaler | creation takes milliseconds; depends on input and output dimensions | once the sizes have been stable for 250 ms |

`grcDevice::Manage` waits until it has observed the same requested size for
20 frames before resetting. The app writes the latest size from its event
loop. On macOS and Windows, that loop stops running during the system's
modal resize loop, so the engine receives the final size after the drag.
Frames continue at the old resolution and are scaled to fit, preserving
their aspect ratio. An SDL event watch runs during the modal loop and
updates the surface size on the renderer.

Only the renderer resizes the Metal layer
(`SDL_VIDEO_METAL_AUTO_RESIZE_DRAWABLE=0`). Allowing SDL to resize it as well
caused frames rendered for one size to appear at another. The current phone
app uses a fixed window and sets its back buffer at startup. Although
`InputBlock::resize` is available there, the memory peak from a device reset
has not been measured on a phone.

The present step scales the back buffer with one of four upscalers (Settings
› Upscaling):

| Upscaler | Where | Input |
|---|---|---|
| Bicubic (Catmull-Rom) | everywhere | the finished frame |
| MetalFX (spatial) | Apple GPUs | the finished frame |
| MetalFX Temporal | Apple GPUs | the scene from several frames, jittered |
| FSR 2 (AMD, through sp-fidelity) | GPUs with the features it needs | the same as MetalFX Temporal |

Upscalers run on the submit thread, on the same queue, after the game's
commands and before the present pass, which draws their result 1:1. The
game's commands and the upscaler's are submitted before the window's next
image is taken: waiting for one does not hold the frame back from the GPU,
and the present pass, drawing into it, is a submission of its own. MetalFX
has command buffers of its own because wgpu does not let raw Metal commands
share its encoder; FSR is wgpu compute passes. Upscalers are made for one
input and one output size; while the sizes change (a window being resized)
frames use the bicubic path, and they are made once the sizes have held for
250 ms. Upscaler changes apply on the next frame. Linking the MetalFX
framework requires macOS 13; the iOS requirement remains iOS 16.

What they cost grows with the window's pixels, not the scene's. From
1280x720 to 2560x1440 on an M1 Pro (`upscale_cost` in `upscale_tests.rs`):
MetalFX spatial about 2 ms a frame, MetalFX Temporal 3-4.3, FSR 5.5-6. On Apple
GPUs the statistics show the upscalers' GPU time a frame (`upscaling ms
GPU`), measured between two empty command buffers around their commands
(wgpu-core gives each pass a command buffer of its own).

The spatial scaler uses perceptual mode to match the gamma-encoded back
buffer, and scales the game's HUD along with the scene; the app overlay is
drawn afterwards.

### Temporal upscaling

A temporal upscaler builds each output frame from several input frames drawn
at different sub-pixel offsets: sharper than one frame scaled up, and
antialiased. It needs four things the engine's DirectX 10 path does not have
(it has no TAA): the scene without the HUD, its depth, motion vectors and a
jittered projection (`glue5-gpu/src/scene.rs`).

- **Where the scene ends.** A hook on `PostFX::ProcessNonDepthFX` (render
  thread) puts a host command in the GPU ring once post-processing has drawn
  the finished scene into the back buffer (`op::SCENE_DONE`,
  `glue5-runtime/src/upscale.rs`). Everything after it is HUD. There the
  renderer copies the scene, converts the scene's depth to `Depth32Float` and
  draws the motion. Frames without the command (menus over everything) use
  the bicubic path, and the upscaler's history restarts after them.
- **The camera.** Every vertex program reads RAGE's `rage_matrices` in slot 1
  (gWorld, gWorldView, gWorldViewProj, gViewInverse; row vectors). The first
  perspective draw of a frame into a scene-sized target with depth gives the
  view-projection (gWorld⁻¹ · gWorldViewProj), the projection (near 0.15 m,
  far 7.5 km, reversed Z) and the camera's position.
- **Motion.** The camera's alone: each pixel's depth reprojected with this
  frame's inverse view-projection and the previous one's (one matrix, made in
  f64). Right for everything that does not move by itself. Vehicles and
  people moving on their own get the camera's motion; the upscalers' own
  checks catch most of it. A camera jump of more than 8 m or 45° resets the
  history.
- **Jitter.** A Halton (2, 3) sequence of offsets within a pixel, 8 × the
  scale² long (8 at 1x, 32 at 2x, 64 at 3x, as FSR does), so each output
  pixel gets samples all over its share of an input pixel. It is
  added to gWorldViewProj in the copy of each draw's constants for draws
  into scene-sized targets whose matrix is a perspective one. Full-screen
  passes (orthographic) stay where they are: lighting and post-processing
  read the scene pixel for pixel, and moving them too would undo the jitter.
  The upscalers are told the offset the scene was moved by. Its sign was
  measured: `upscale_tests.rs` draws a known pattern jittered, upscales 48
  frames through the real paths and compares them with the pattern
  supersampled. Jittered, MetalFX's error drops from 0.0095 to 0.0022 and
  FSR's from 0.0083 to 0.0010; with the opposite sign MetalFX's doubles.

Temporal upscalers scale by 3x at most: MetalFX's temporal scaler takes no
more (it crashed at 4x), and FSR is made for that much. Below a third of
the window (the render scale goes down to 25 %), they make 3x the scene and
the present pass's bicubic filter scales the rest.

The history goes on from frame to frame while each has its scene. It did
not at first: the renderer looked for the previous frame's scene after the
present had taken it, so every frame restarted the history (and gave the
upscalers no motion). They showed each frame alone, jittered: at a 25 %
render scale the scene shook by up to two pixels of the window.

The HUD goes back over the upscaled scene as what it changed in the back
buffer: the final back buffer minus the scene copy, both through the bicubic
filter, added to the upscaled scene. Where there is no HUD that adds
nothing; under a translucent panel the scene stays sharp. The HUD itself
does not enter the upscalers' history, so it leaves no trails.

FSR comes from sp-fidelity: the FidelityFX SDK's FSR 2.3 ported to Rust
and WGSL, with a wgpu backend (`glue5-gpu/src/fsr.rs`). It needs device
features the renderer asks for where the GPU has them (subgroups, texture
atomics, 16-bit normalized formats, half floats for its FP16 passes), and
is offered only where it has them all. Making it compiles its passes (5 s
on an M1 Pro), on a thread of its own; frames use the bicubic path
meanwhile. Its accumulation pass is most of its time (3.8 of 5.4 ms of
passes, 1280x720 to 2560x1440 on an M1 Pro). It has no frame generation.

Before it FSR was AMD's FSR 3 upscaler through the wgpu-ffx crate, SPIR-V
passes translated by naga. It needed patches to run on Metal at all, took
16 ms a frame on the M1 Pro (10 in its accumulation pass), and its
accumulation pass read the wrong motion vectors (wgpu-ffx's open pull
request #28: trails in motion).

### Frame generation

With MetalFX Temporal on macOS and iOS 26, the frame interpolator makes a
frame between the temporal scaler's last two from this frame's depth and
motion (the scaler writes its frames into two textures in turn). The submit
thread shows it first, then the new frame, each for half the frame time
(the frame rate set, or the measured time between the game's frames): twice
the frames on screen for the game's work of one, for half a frame more of
latency. The HUD goes over the generated frame as over the others (the
newer frame's).

It works on an iPhone 16 Pro (iOS 27): the generated frame lies halfway
between its neighbours (as far from each), without double edges, and frames
stay 16.7 and 33 ms on screen in turn at a game rate of 30. But it costs
more than it gives there. At a render scale of 0.6 (1574x724 to 2622x1206)
and 30 frames a second, the upscalers' GPU time a frame was:

| | Upscaling GPU | Game |
|---|---|---|
| Bicubic | - | 30 fps |
| MetalFX spatial | 4.6 ms | 30 fps |
| MetalFX Temporal | 7-11 ms | 30 fps |
| MetalFX Temporal + frame generation | 27-40 ms | 15-20 fps |
| the same, output 1956x900 | 16 ms | 23 fps |
| the same, output 1574x724 (no upscaling) | 16 ms | 24 fps |

The interpolator alone takes about 12 ms at 1574x724 and 20 ms at the
screen's size; the GPU it takes from the game lowers the game's rate more
than the generated frames add. Giving it the temporal scaler (its
descriptor's `scaler`) changes nothing. It pays only where the GPU has that
much to spare each frame: a game held back by the CPU, or a lower render
scale.

## Frame pacing

A display faster than the game shows each frame at the first refresh
after it is ready. On a 120 Hz ProMotion phone or a 144 Hz monitor, a game
running at 40-50 fps then shows frames for one, two or three refreshes in
turn: judder, although no frame is late. With a frame rate set (Settings
› Frame rate), each frame stays on screen for at least its share of a
second, counted from when the previous one appeared. On Apple platforms
the renderer takes the window's drawables from its Metal layer itself and
presents them with `presentDrawable:afterMinimumDuration:`, which wgpu's
present does not offer. Held drawables also slow the engine down to that
rate. Elsewhere, frames are not paced and the engine's limiter works alone.

The statistics report how long frames stayed on screen (`on screen ms`,
from the drawables' presented times) and how often that time changed.
On an iPhone 16 Pro in the prologue (2,500-3,300 draws per frame, the phone
hot), frames at 60 stayed 16.7, 25 or 33 ms (200-280 changes in 10 s); at
30 every frame stayed 33.3 ms. 40 (every third refresh) is offered on
120 Hz displays only, and is even only while the panel runs at 120 Hz:
iOS lowers it to 60 Hz when the phone is hot, and 40 then becomes 33 ms.
The engine's own limiter (`-frameLimit`, in 60 Hz intervals) stays: it
saves the CPU time of frames that would wait anyway.

The engine queues frames for the GPU (`FrameQueueLimit`, 1 in
`CSystem::RenderThreadInit`). With one, its render thread waits for the
previous frame to finish ("Wait on queued frames") before it starts the
next. Glue 5 passes `-FrameQueueLimit=2` on desktop and Android, at the
cost of one frame of latency. The game's benchmark ran 11-32 % faster on
Windows. On a Pixel 8a it ran 8 % faster (two runs each way, the phones
cooling in between). iOS keeps one: two brought no gain on an iPhone
13 Pro or an iPhone 16 Pro (59.9 and 54.3 fps with one, 55.5 and 56.1
with two, the phone warming over the four runs). There the renderer never
waits for the GPU and the CPU sets the pace, so a second frame only adds
latency. `GLUE5_ENGINE_ARGS=-FrameQueueLimit=N` overrides it.

A phone that heats up holds the game to fewer frames a second before the
system's own governor does. Android's Thermal API is read (the NDK's
`AThermal_getCurrentThermalStatus`, and `AThermal_getThermalHeadroom` for
the forecast, both looked up at run time), and a governor with hysteresis
turns the readings into a level (0 to 3): it rises after the heat has been
asked for a few seconds and falls one level at a time, slowly, so a phone
near a threshold does not flap. The host's limiter then waits for the
slower of the engine's own limit and the level's (40 fps at level 1, 30 at
levels 2 and 3), the performance hint's target follows it (so the system
does not raise clocks for a rate the host is not keeping), and the display
is asked for the same rate. With *Frame rate and resolution*, the last
level also renders at 85% of the chosen scale. The setting is in Settings
› Video › *Thermal back-off*, on by default on Android and off elsewhere.

## Input

Input is routed through the game's PC interfaces to preserve controller
prompts, aim assist and menu behaviour:

- **Keyboard and mouse** go to the engine's input block, exactly as the
  browser page fed it (virtual-key codes, relative mouse with pointer lock).
  Same on desktop and on tablets or phones with a keyboard.
- **Controllers** (SDL: XInput, PlayStation, Switch, Apple's GameController,
  Android) feed the engine's own pad code: the `pad_update` hook writes RAGE's
  pad state after `rage::ioPad::Update`, where XInput would on PC. The game
  then shows controller prompts by itself.
- **Touch controls** are a virtual controller in pad slot 0, laid out like
  one held in landscape within the safe area: a floating movement stick and a
  D-pad cross on the left, a camera stick (the right stick) and the face
  buttons on the right, triggers and shoulder buttons in the top corners.
  They are smaller on phones. They are shown on touch devices while touch is the input in use
  and hide as soon as a controller, keyboard or mouse is used (setting:
  Auto / Always / Off). Visibility follows the last input used, so an idle
  paired controller does not hide the touch controls.
- **Text entry**: hooks on `CTextInputBox::Open/Close` tell the app when the
  game wants text (save names, cheats). SDL text input runs only while the
  box is open. On touch devices it brings up the system keyboard; typed text
  goes into the same input block as a physical keyboard. The system keyboard
  handles languages, dictation and accessibility, and stays hidden when a
  hardware keyboard is attached. On desktop it hands keys to the system's
  input methods: outside the box their accent menu (a held key on macOS),
  dead keys and IME windows would get in the way of play.

## Mods

Mods are Luau scripts (`crates/glue5-mods`) that call the game's script
commands (natives). Each mod is a folder in the mods folder with a
`main.luau` and an optional `mod.json`: its name as shown, description,
author, version, whether it starts with the game and restarts when a
file is saved. The Studio shows mod.json as the mod's page, not as a
file. Its other files may be in folders: `require` takes a path from
the file that asks (`./name`, `../core/name`), as Luau's require by
string does and as the Studio's type checker resolves it, and stays in
the mod's folder. The mods folder is the user directory's `mods` on computers and
beside the game's folder on phones (`Documents/mods` on iOS, where the
Files app reaches it); one from before is moved there.

The Studio exports a mod as a .zip (its files, not what it stored) and
imports one (its files at the top or in one folder, with a main.luau;
a name taken gets `-2`); an imported mod does not start with the game
until the player says so.

**Mods that come with GLUE 5** (Glue Trainer) are the repository's
`mods/`, built into the app (glue5-mods' build script embeds their files)
and copied into the mods folder when it opens (`bundled::install`): one
code path for every platform, nothing to package per store, and once
there they are mods like any other, which the Studio opens and players
can change. A note in the folder (`.glue5/bundled.json`) keeps each
installed file's hash, so updates go as a package manager's do with
configuration files:

- no folder (never installed, or deleted): the mod is installed as built;
- every file as installed: the new version replaces them, adds its new
  files and deletes those it dropped;
- a file edited or deleted (or a folder of the player's with that name):
  nothing is touched, and the player hears once per version that a newer
  one is there for whoever deletes the folder.

`mod.json` is not compared: the new one is written with what the player
chose in it (starts with the game, restarts on save). What the mod
keeps (its `settings.json`, storage and database) is never one of its
files. The alternative, running them read-only from the app, would hide
them from the Studio and from players who want to learn from them or
change them.

The Mods page holds them: the start page's Mods button, Settings › Game ›
Mods, F7 in the game, and on touch screens a button beside the settings
button while a mod has a menu. Its switch turns mods on; the folder opens
at once (the page lists the mods, the Studio edits them before the game),
and they run with the game started next, which installs their hooks
(`open` and `install` in glue5-mods).

**Luau** is a typed Lua (gradual and `--!strict`), embedded with `mlua`. It
was chosen because:

- its interpreter is the fastest of the typed options without a JIT,
  which iOS forbids. A call from Luau into Rust costs about 40 ns, against
  about 76 ns for QuickJS;
- it has a 3-component `vector` type, which matches the game's `Vector3`;
- it has a sandbox, memory limits, and an interrupt that can suspend a
  running script.

**The natives come from the game module.** glue5-game's build script reads
every `rage::scrThread::RegisterCommand(hash, handler, name, file,
signature)` call in game.wasm: 8,561 natives with their name, group, hash
and handler (a function table index). The hashes are this build's own, so
everything is keyed by name. Where the handler calls a typed
`<group>_commands::Command<Name>(...)`, the C++ parameter types come from
its demangled name; vectors returned through a pointer show in its wasm
signature.

glue5-mods' build script joins that table with the community's natives
database (`third_party/nativedb`, downloaded by `mise run setup`, pinned,
not in git because it has no license). The database adds:

- parameter names;
- documentation;
- handle types (`Ped`, `Vehicle`, ...);
- which pointers are outputs.

The binary keeps the final say on calling conventions. For example, the
database's `x, y, z` floats become one `vector` where the C++ command takes
an `rage::scrVector`.

The build generates two things:

- a compact table sorted by namespace and name;
- `natives.d.luau`, the Luau definitions, named the Luau way:
  `vehicle.createVehicle(model, coords, ...)`, with handle types.

Output parameters come back as extra return values, and a handle of 0 comes
back as `nil`. An output is one value: natives that take a structure or an
array through a pointer (the database's `Any*` where the command takes an
`int&`), or keep the pointer after the call (save blocks, broadcast
variables), are left out.

**Each mod runs in a script thread of its own**, as the game's scripts do.
Its program is built in wasm memory: a copy of a loaded `scrProgram` with
its own name, id and code (`ENTER; loop: PUSH_0; NATIVE WAIT; J loop`),
registered with `scrProgramRegistry::Insert`. The thread is started with
`CTheScripts::GtaStartNewThreadWithProgramId`.

The engine calls WAIT every frame for every waiting thread, and a hook on
`rage::scrThread::Wait` resumes the mod's Luau coroutine when the mod's
thread is about to wait. Luau's `wait(ms)` becomes WAIT's argument. This
gives every native a real script context, the same as a vanilla script's
natives:

- the active thread is the mod's;
- entities the mod creates belong to its handler;
- `scrThread::KillThread` cleans them up when the mod stops.

**In GTA Online a mod runs as the session's script.** A mod's thread is not
a network script: CREATE_VEHICLE with isNetwork returns 0 from it, and
NETWORK_SET_THIS_SCRIPT_IS_NETWORK_SCRIPT quits the game for a script that
is not in its allowlist of 378. So while freemode runs (the session script
every player runs), each mod turn runs with freemode's `GtaThread` as the
active thread (`tls + 3088`, swapped for the turn and restored after, a
panic too):

- what the mod makes with isNetwork is networked, as freemode's own: every
  player sees it, with or without the mod, and sees the player in it;
  natives that need a network script (explosive rounds...) work too;
- natives about the running script (`*THIS_SCRIPT*`, `*THIS_THREAD*`,
  `PARTICIPANT_ID*`) run with the mod's own thread active: its name, id
  and host are the mod's;
- what it made as freemode (handles returned by `CREATE_*`, `CLONE_*`,
  `ADD_BLIP_*`) is given back when it stops: entities no longer needed,
  blips and pickups removed, as the engine does for a script that ends;
- no list of natives to keep: the rule is the turn, and the cost is two
  stores per turn and finding freemode once a frame.

In a job (no freemode) and in story mode the mod runs as itself.

Starting and stopping happen in the same hook, from whichever thread waits
next, so all of it stays on the engine's script update. A mod's code starts
once the player is in the game: no loading screen, screen faded in, player
playing.

**A mod must not take the game down.**

- Errors stay in the mod's VM: the mod stops with the message and its
  traceback.
- Arguments are checked before the native runs. For an entity handle the
  check is what `fwScriptGuid::GetBaseFromGuid` does, without its
  `unreachable` for an index past the pool: a stale or made-up handle is an
  error in the mod, not a trap.
- Every native is called through `glue5_call_native`, which catches a trap
  (`GLUE5_TRY`) and restores the wasm stack pointer. The trap becomes an
  error in the mod, and the runtime is marked as possibly damaged.
- A mod runs at most 2 ms before the interrupt suspends it until the next
  frame. One that runs 2 s without calling `wait` is stopped.
- A mod's script thread is ended only by the hook, from outside it:
  TERMINATE_THREAD and TERMINATE_ALL_SCRIPTS_WITH_THIS_NAME refuse the
  mods' threads (ended from inside, the hook would go on in a dead one).

### Menus, tasks, the world, data

- **Menus**: `ui.menu(title)`, with buttons, toggles, sliders, numbers,
  choices, multiple choices, text, colours, labels, separators, progress
  bars and submenus (at any depth). The application draws them with its own
  widgets (touch and controllers too): the Mods page's MENUS (see above
  for how it opens), a menu at a time with a back stack, never flattened.
  - The menus live in the shared state (glue5-mods `menus/`, under a
    lock), outside the mod's memory; the Lua handles are views of them.
    The app, the Studio and MCP read one snapshot (`menus()`, an `Arc`
    rebuilt only after a change) and never touch a mod's Lua.
  - A player's change (`menu_input`) is checked against the item (kind,
    range, enabled and visible) and written to the shared value at once,
    so every view agrees; it is queued for the mod, one pending value per
    item (a dragged slider coalesces) and up to 64 presses. At its next
    turn the mod takes the queue: each handle's cached `value` changes and
    the callback runs as a task, so `value` and the callbacks agree. A
    mod's own assignment drops a pending value for that item and calls
    nothing: the last write wins. One that changes nothing leaves the
    snapshot and its revision as they were, so a mod can show the game's
    state by setting its items every frame.
  - A menu's `search` is the viewers' own: each filters the rows it
    draws by what its player typed (the app keeps it with the level, so
    going into a row it found and back keeps it); the mod hears nothing.
  - Viewers: the app, each Studio session and MCP's `menu_open` count as
    looking at a menu; the first opens it (`onOpen` runs), the last one
    leaving closes it (`onClose`). `menu:show()` asks the app to open the
    page at a menu, at most every 2 seconds.
  - `ui.notify` shows a notification over the game.
  - A mod with a menu stays running after its main code returns.
  - A run of a mod has 4096 menus and items and 65536 options at most,
    all together (every change copies the snapshot the app reads each
    frame), and titles, labels, notes and options are cut short; a log
    line is cut at 8 KB.
- **Tasks**: a mod's coroutines are scheduled by game time
  (`GET_GAME_TIMER`, so they stop while the game is paused):
  - the main chunk;
  - `task.spawn` and `task.delay`;
  - menu callbacks.

  The mod's script thread waits as long as the earliest of them, or one
  frame while it has menus. The `task` table is the TASK_ natives' one too.
- **Input** (input.rs): `input.isDown`, `pressed`, `released`, `onKey`,
  `mousePosition`, for what ScriptHookV's keyboard handler gave mods, and
  `keys` (every name), for mods' key bindings.
  - The app reports keys and mouse buttons where it hands them to the
    game (the game's input block takes the same keys, plus generic
    modifier codes and aliases the mods do not see): events in a bounded
    queue, under a lock held for a push. Pad 0's buttons come from the
    runtime's pads (`take_gamepad_changes`: pressed and released since the
    last read, so a tap between two frames counts). A mod takes what came
    since its last turn as the turn starts, on the script update; its
    handlers run there as tasks, in order, never on the app's thread.
  - Keys are physical, named as on a US keyboard (as the game's controls
    take them); mouse buttons and the wheel, and the pad's buttons by
    position (SDL's names), share the one `Key` type.
  - What mods see is what the game gets: nothing behind a page of the app
    (everything is released when one opens or the window loses the
    focus), no presses while the game's text box takes text. Nothing is
    taken from the game: a key reaches the game before any mod runs, so a
    mod takes over a control the game's way, `DISABLE_CONTROL_ACTION`
    each frame. On touch screens the touch controls are pad 0.
- **World**: `world.vehicles()`, `peds()`, `objects()` walk the engine's
  pools and give script handles (`fwScriptGuid::CreateGuid`), while the
  handles' pool keeps room.
- **Names from the installed game**: the engine keeps the strings it
  hashed (`atHashStringNamespaceSupport`). Once the game has loaded, the
  catalog is read:
  - vehicle and ped models from the archetype store;
  - weapons from `CWeaponInfoManager`.

  It becomes `world.d.luau`, string unions (`VehicleModel`, `PedModel`,
  `WeaponName`), saved for the next start. Natives take them where a
  parameter is a vehicle's or ped's model or a weapon, so a misspelt name
  is a type error ("did you mean ...?"). The names come from the assets
  the game loaded, whatever their version; none is in the build.
- **Data**: `json.encode`/`json.decode` (json.rs: arrays are tables with
  keys 1..n or none, null is nil, functions and cycles are errors; the
  text is written as the tables are walked and stops at 1 MB, so a table
  held many times over cannot make it endless) and
  `storage` (storage.rs): a mod's values by key, JSON, 1 MB, written
  within two seconds and when it stops, in the mods folder's
  `.glue5/storage/<mod>.json`, so a mod handed on carries no player's
  data. Luau's `buffer` is there too; files and the network are not
  (HTTP, with sites a mod declares and the player approves, is planned).
- **Drawing** (draw.rs, glue5-runtime's sprites.rs, glue5-gpu's
  sprites.rs): what ScriptHookV's createTexture/drawTexture gave, grown to
  what HUDs need. The natives already draw rectangles, the game's own
  sprites (streamed texture dictionaries) and text inside the game's
  frame; what they cannot is an image of the mod's own, and anything over
  one.
  - **Calls are the hot path** (thousands a frame for a busy HUD):
    positions and sizes are positional numbers, a colour one packed
    number (0xRRGGBBAA), the rest a style made once (`draw.style{...}`,
    userdata) and passed again. Measured (release, M1 Pro, 100,000 calls
    of a rectangle): an options table written in the call 425 ns a call, the same table
    made once 332 ns (its keys are looked up by string), a packed colour and
    a style 104 ns, a packed colour alone 65 ns. The call records into the mod's list (no
    allocation once warm: the lists, their points and groups are kept
    from turn to turn; a text is the same `Arc<str>` while its Luau
    string is the same).
  - **Turns**: what a mod draws in a turn shows from the end of that turn
    until the end of its next whole one; a turn cut short by the 2 ms
    slice goes on in the next, its drawings with it (no half a frame on
    screen). At each engine frame, when a mod drew anew or one stopped,
    the mods' lists become one, by z (glue5-runtime's `sprites`); the
    list the renderer is done with comes back to be filled again.
  - **Images**: `draw.loadImage` reads a PNG of the mod's folder (the
    Studio's paths, checked the same way) and decodes it on a thread of
    its own, refused from its header past 2048×2048 or the 32 MB a run
    of a mod may hold; its smaller levels (mipmaps) are made there too,
    and count. It is uploaded the first time it is drawn (its pixels
    leave the CPU) and freed when nothing holds it. A sprite sheet's
    frame (`uv`) and nine-slicing are texture coordinates: no texture
    more.
  - **The renderer** lays the latest list out with epaint (egui's painting
    crate, no egui frame): its tessellator anti-aliases by a pixel of
    feathering. All of it is one mesh, cut into draw calls only where the
    texture changes in the drawing order (shapes and text share the font
    atlas, a sprite sheet is one texture) or a clip does. Additive
    drawings are their colour without alpha: the premultiplied blend then
    adds them, in the same pipeline and batches. Meshes, scratch paths
    and egui's GPU buffers are kept and only grow; a list is laid out
    again only when a new one comes, the window's size changes, or it has
    labels in the world. Texts are laid out once while the same string,
    size and font are drawn; in a scaled group (a label in the world) at
    steps of size 2^¼ apart, stretched to theirs, so a label coming nearer
    is laid out again every 19 %, not every frame; the fonts (the app's and the mods', 4 a mod)
    share one atlas of 2048 pixels' side. What falls outside the screen
    or its clip is not tessellated. It is drawn into the window's image
    after the game's frame is scaled to it, before the app's UI: at the
    window's resolution, over upscaled and generated frames alike.
  - **Groups** (`draw.push`, `draw.clip`, `draw.pop`) place what follows
    (moved, scaled, turned) and clip it, with no function call in
    between, so a long list in a group can be cut by the time slice like
    any other drawing.
  - **The world**: `draw.push3d` and the `3d` calls anchor a group at a
    point of the world. While the list has one, the renderer asks the
    command thread for each frame's camera (the view-projection the
    temporal upscalers read, scene.rs) and projects the anchors when that
    frame is composed: the label moves with the image, not a frame
    behind it. Points behind the camera or past `maxDistance` are culled
    before anything in their group is tessellated.
  - **Cost**, measured on an M1 Pro, headless at 1920×1080: with mods
    that do not draw nothing runs (no layout, no pass; frames as without
    mods). A stress mod (5,000 shapes, 500 texts, 300 sprites, 100 labels
    in the world, every frame) takes 1.8 ms of its 2 ms slice for its
    calls; the renderer lays its list out in 2.4 ms alone (the ignored
    `sprites_cost` test: tessellation 1.3-1.4 ms, buffer copies 1.0 ms),
    4.3-5.8 ms on its thread in the game, and draws it in 1 ms of GPU,
    in 3 draw calls; the game went from 5.4-5.8 ms (p50) to 7.0-7.6 ms a
    frame. The anti-aliasing feathering is about half of that (without
    it: 1.3 ms CPU, 0.4 ms GPU), and stays. A HUD (gauge, a clipped list
    of 30 texts, a sprite sheet, a label in the world) takes 0.2-0.3 ms
    a list. On a phone, three to four times slower, the stress list
    would take some 8-20 ms of the renderer's thread: HUDs of hundreds of
    drawings are the size to aim at, and a list without labels in the
    world is laid out only when it changes.
- **Stability**:
  - an entity of the wrong kind (`CEntity` +40: 3 vehicle, 4 ped, 5
    object) is an error before the native runs;
  - natives that create entities are refused when the pool is nearly
    full;
  - a frame draws skeletons within fixed budgets (400 matrix sets in the
    command buffer, 512 dependencies in the draw list manager); past them
    the engine stopped (some 114 peds in view). Peds and vehicles that
    would not fit are now skipped for that frame through the engine's own
    `CEntityDrawHandler::ShouldSkipEntity` (runtime `draw_lists.rs`), for
    the game's crowds as well as mods';
  - mods' script threads get stacks of their own (gameconfig.xml's MICRO
    stacks, 20 → 52);
  - when the game stops working, the failure names the mod and the native
    that ran, and that mod does not start by itself next time;
  - a panic in GLUE 5's code for a mod (a callback, a conversion) is
    caught in the WAIT hook: that mod stops with an error, the game goes
    on (a panic crossing into the engine would abort it); the Studio's
    threads may panic without stopping the game either;
  - a frame gives all the mods together 4 ms, each at most 2 ms (its main
    code and tasks share it, taking turns going first); a mod that finds
    the budget spent goes first in the next frame.

### GLUE 5 Studio

The app serves an IDE for mods while mods are on and the Mods page's
STUDIO switch says so, before the game too. By default only a browser on the same device can
use it; "Other devices" opens it to the network (the default on phones,
whose own browser is no place to write mods).

- **Server** (`glue5-mods/src/studio`): plain `std::net`, a thread per
  connection; HTTP/1.1 for the page, WebSockets (`tungstenite`, no async)
  for the Studio and the type checker.
- **Pairing** (`studio/pairing.rs`): the code (8 characters of Crockford
  base32, 40 bits, kept in the settings) is exchanged once for a token of
  the client's own (`POST /pair`): 32 random bytes, which a page keeps in
  its origin's storage and sends as `?token=` or `Authorization: Bearer`.
  The game keeps each token's SHA-256 only, with the client's name (from
  its User-Agent), its scope and when it was last used, in the mods
  folder (`.glue5/pairings.json`): pairings outlive the game's runs and a
  new code; the Mods page lists and removes them, which closes their
  connections within a second; one unused for 90 days goes.
  - Storage rather than a cookie: an origin's storage is its own (a
    cookie goes to every port of the host), and a token no other page can
    send means no request forgery to guard against.
  - Scopes: the Studio's pairings may do everything; the control page's
    only list the mods, start and stop them and use their menus (no files,
    no eval, no type checker, no MCP), so a phone paired for the menus is
    no key to the game's code.
  - The code itself still works as a full credential, for agents and
    tools that keep no pairing (whoever has it could pair anyway); the
    Studio gives each agent a pairing of its own.
  - A wrong code or token costs its connection a second (no lockout that
    a page elsewhere could trigger). WebSockets must come from the page's
    own origin, since a page elsewhere in the same browser could
    otherwise drive the game through localhost.
  - The address is the device's network address and the port: a page's
    pairing lasts as long as they do (a new address is a new origin to
    pair).
- **Control page** (`/control`, `control.rs`): the same wasm, mounted for
  the mods and their menus (a mod's in the address, `?mod=`), a phone's
  layout one level at a time and a wide screen's side by side. The
  editor's module (CodeMirror) is loaded only by the Studio, the first
  time it shows (`/editor.js`, its class a global the wasm calls), so the
  control page fetches none of it. A web app manifest lets a phone keep
  it on its home screen.
- **Type checker in the game**: Luau.Analysis is compiled from Luau's
  sources (`third_party/luau`, the release mlua builds the rest from) with
  a small C API (`csrc/checker.cpp`). It loads the natives' definitions in
  about 80 ms; checking a file takes under a millisecond. It exists only
  while an editor is connected (some 30 MB, which the log reports) and
  speaks LSP over a WebSocket (`/lsp`): diagnostics, completions, hovers
  and signature help (the call's parameters, the current one marked).
  Completions are filtered by what
  is typed before they are sent, so lists stay short on phones. Types are
  printed in full and the host writes the catalog's unions as their
  names (`VehicleModel`...), so a signature stays one line. On an iPhone
  16 Pro it loads in 1.5 s and takes 76 MiB. A mod's `mod.json` is
  checked by the host (JSON, an object, `autostart` true or false).
- **The page** (`crates/glue5-studio`) is Leptos compiled to wasm, around
  CodeMirror 6 with `@codemirror/lsp-client`. No node: the crate is its
  own workspace, built for `wasm32-unknown-unknown` by glue5-mods' build
  script (a cargo of its own, `wasm-bindgen` as a library). CodeMirror's
  modules come from npm by `mise run setup` (pinned by their integrity
  hashes) and load through an import map, without a bundler. Everything
  is embedded in the app, gzipped; ETags make the browser revalidate.
- **MCP** (`POST /mcp`, JSON-RPC without streams) gives coding agents the
  same functions as the Studio:
  - write a file and get its type errors back;
  - start or restart a mod and get its first log lines back;
  - read logs;
  - evaluate Luau in the game;
  - search the natives;
  - use a mod's menus (`menus`, `menu_input`) as a player would.

  Any client that speaks Streamable HTTP with a header works: the
  Studio's welcome page has each known client's own steps (agents.rs,
  checked against their documentation in October 2026: a command, or a
  JSON or TOML file; Cursor's and VS Code's install links), makes the
  agent a pairing of its own (`pairings.agent`) and fills its token in.
  Claude Desktop goes through `mcp-remote`: its connectors reach servers
  from the cloud, never this computer. The snippets are coloured by a
  highlighter of the page's own (highlight.rs, bash/JSON/TOML): a
  read-only snippet needs no editor.
- **Eval** runs in a mod of its own (`~eval`): a script thread whose Luau
  takes snippets from a queue. The console's input is a one-line
  CodeMirror with the same language server (a document of its own,
  `~console/input.luau`): highlighting and completions as in a file.
- Files written by any client reach open editors (a WebSocket message),
  changing only what differs (the cursor stays); a tab with unsaved
  changes keeps them and says so. Saving a .luau file restarts its
  running mod when its page says so (restartOnSave, on by default),
  whoever saves it: the Studio or an agent's write_file, which then
  answers with the new run's first log lines. Each run of a mod owns
  its menus, so the old run's going does not take the new run's.
- A mod's files show as a tree of its folders. Folders hold files: a new
  one is made with its first file; renaming moves a file or a folder
  within its mod (making the folders it goes into), deleting takes a
  folder with what it holds, and folders left empty go. Tabs follow what
  moved.
- A page that loses the game retries on its own; it asks to pair again
  only when the server says its token is no more (`/check`), and keeps
  its editor, tabs and history. Request bodies are read only from whoever
  has a credential (before it, only `/pair`'s small one), a few connections at
  a time.
- Luau's indentation in the editor: a line leaving blocks or brackets
  open adds a level, however many (`f(x, function()` ... `end)`), and
  if-expressions open none.
- The mods' menus show on the control page: the same items the game
  draws, sent again when they change; what is done there reaches the mod
  as from the game. A right click on a mod in the Studio opens it there.

## Known limits

- The renderer handles missing mobile GPU features as follows:
  - no BC texture compression (iPhones before the A17 Pro, most Android
    GPUs): textures are decoded on the CPU (slower uploads, more memory),
    large ones without their first level;
  - no filtering of 32-bit float textures (many Android GPUs): programs
    that sample one get a variant that reads it unfiltered;
  - no indirect draws: those draws are skipped.

  `GLUE5_NO_BC=1` and `GLUE5_NO_FLOAT32_FILTER=1` force the first two on a
  desktop GPU, for testing.
- Graphics quality is set in the game's menu (Settings › Graphics). The
  app omits the WASM build's command-line quality switches because they
  would override the menu at each start. App settings control back buffer
  size (`-forceResolution`), frame rate (`-frameLimit`), memory pools and
  touch controls.
- The menu's data is older than the engine. The game ships the base
  `pausemenu.xml` and texts, without the PC release's later menu, so
  Settings › Graphics lacked Population Variety, Reflection MSAA,
  Particles Quality, Grass Quality, Motion Blur Strength and In-Game Depth
  Of Field Effects, and there was no Advanced Graphics screen. The engine
  still handles all of them: it reads, applies and saves them, and asks for
  a restart where the PC game does (grass, particles, variety). The
  overlay of `pausemenu.xml` adds those items and the Advanced Graphics
  screen (long and high-resolution shadows, high detail streaming while
  flying, extended distance scaling and shadow distance), and `texts.rs`
  gives their names in English, which the texts lack. Frame Scaling Mode is
  left out because the app sizes the frame, and TXAA because it is NVIDIA's.
  Before this, grass, particles and variety kept whatever `settings.xml`
  held, such as the low-memory preset's values.
- MSAA stays off, whatever the game's menu says. The game's shaders are
  its DirectX 10 set (`win32_40`), whose multisampled effects are an older
  revision than the code that drives them: a depth resolve pass and ten
  post-processing passes are missing, and the tiled lighting's
  multisampled depth reduction has no translation. With MSAA on, the
  render thread trapped at the first frame of a scene, and post-processing
  drew the wrong passes into a black image. The engine turns MSAA off
  itself on DirectX 10 (`-DX10`); hooks on `CSettingsManager::Load` and
  `RequestNewSettings` do the same for the settings loaded and those the
  menu asks for. Reflection MSAA (water) works and is left as chosen.
- Shader quality Normal runs as High. Normal loads the effects from
  `win32_40_lq`, older than the engine: its `postfx` lacks
  `DistortionParams`, which the engine requires (it stops a few seconds
  in), and few of its programs have a translation. The same hooks raise
  it to High (`shader_settings.rs`).
- The low-memory profile defaults on for phones and tablets with less than
  8 GB of RAM. It uses smaller pools and lowers texture quality, LOD,
  density, variety and shadows to the WASM build's low preset. The normal
  profile uses about 4 GB. `game_settings::apply_low_preset` writes the
  graphics preset to `settings.xml` once; later changes use the game's menu.

  The resource heap is capped at its starting size (`gameResourceMB`,
  768 MiB in low memory), on phones also without low memory (the game's
  own size). The PC build grows it by 256 MiB whenever the streaming engine
  asks how much memory is left, and the streaming engine keeps it full: in
  the game's benchmark it went from 1.3 to 2.6 GiB, and 8 GB phones with
  low memory off were killed before its end. This can retain resources from areas the
  player has left and exhaust available memory on phones. Capping the heap
  makes the streaming engine evict resources when it fills, as on consoles.
  Setting it too low also causes problems: at 512 MiB, the prologue never
  finishes loading.
- The engine's shutdown waits indefinitely for work that never arrives.
  Glue 5 hooks `CSystem::WantToExit` for Exit Game and `rage::rlShutdown`
  for restarts requested from within a frame. The app then exits or, on
  desktop, relaunches. The engine's restart-pending flag distinguishes the
  two cases; its settings menu sets that flag after saving.

- Memory on phones, low memory off, the game's benchmark (14 October
  2026): an iPhone 13 Pro takes 2.9 GB, an iPhone 16 Pro 3.8 GB, of which
  ~1.5-1.8 GB the engine's memory (its resource heap, mostly), ~0.9-1.5 GB
  Metal's, ~0.5 GB Metal's own CPU-side memory and ~0.3 GB malloc. The
  engine keeps its vertex and index data after the GPU has its copy
  (~300 MB on the 13 Pro; it reads them on the CPU), not its textures'.
  Saved on our side: the shader text kept compressed and one function
  table for all threads (malloc 557 MB instead of 768 on a Mac, ~80 MB on
  a Pixel 8a), small vertex and index buffers in shared GPU buffers on
  Apple GPUs (60-100 MB on the iPhones: each Metal buffer costs memory of
  its own however small it is), Vulkan command pools giving memory back.

## Future work

- **Motion of moving objects**: the motion vectors are the camera's
  (see Temporal upscaling). Per-object motion needs each draw's previous
  world matrix (and skinned peds' previous bones), matched across frames.
- **FSR 1** (EASU + RCAS, plain shaders) in the spatial slot on GPUs
  without MetalFX.
- **Shader adaptation on naga's IR**: the translated WGSL is adapted as
  text (regular expressions: constant buffers packed, interpolation
  copied, depth reads, storage formats), checked against all 4919 shaders
  by a test (`cargo test -r -p glue5-gpu -- --ignored`). Doing it on the
  parsed module and handing wgpu the IR would be sturdier against a new
  game build's shaders.
- **Give freed wasm memory back to the system.** The wasm memory is only
  reserved (16 GiB of address space); its pages count once the engine
  writes them, and then for good: wasm has no `munmap` or `madvise`, so
  memory the engine frees stays in the footprint. Hooks on
  `rage::sysMemGrowBuddyAllocator::Free` and on large frees of the game's
  malloc could hand whole freed pages back (`MADV_FREE_REUSABLE` on Apple
  platforms, `MADV_DONTNEED` elsewhere): a resource heap grown past its
  budget keeps up to 256 MB touched but free.

[egui]: https://github.com/emilk/egui
