# Future work: performance

What is left to make the renderer faster, measured but not done yet. The
numbers are from a Pixel 8a (Mali-G715, Vulkan) running the engine's
benchmark, unless said otherwise. Where things stand (October 2026): about
39 fps over the whole benchmark, against ~28 for the build before the mods
work; in the heaviest scenes the GPU now takes 20-28 ms a frame, so those
are GPU-bound, and the CPU's busiest threads are the engine's own.

## GPU side

- **Render passes split by copies and the like.** About 55 passes a frame
  in heavy scenes, ~20 of them only because a copy, a clear of another
  target, a mip generation, a readback or a dispatch came between draws to
  the same targets. On a tiled GPU (Mali, Adreno, Apple) each split stores
  the targets to memory and loads them again. Measure first (how much
  bandwidth a split costs), then move such commands out of the way when
  the pass does not depend on them.
- **The renderer's own passes**: mip generation (12-14 a frame), depth
  copies for shaders reading depth as colour, pass-through blits: ~1 ms a
  frame together. Small, but entirely ours.

## CPU side: the renderer

- **Command pools on Vulkan.** After the compute-pass fix, the thread that
  frees command buffers still spends over half its time in the driver
  unmapping their memory: our pool policy (TRANSIENT pools, wgpu-hal
  patches 0003 and 0006) makes the driver allocate and free it again and
  again. On an Adreno 750 the same cost was the main reason for the lag
  (keeping the memory doubled the frame rate there, but with no bound it
  took 1.9 GB more). A per-vendor policy with a hard bound would keep a
  little memory for the gain; on Mali the gain is modest. Only worth it if
  the memory stays small.
- **Object lookups in wgpu-core**: every `set_vertex_buffer`,
  `set_index_buffer` and `set_bind_group` resolves an id under a lock and
  clones an `Arc`: 16 % of the command thread. Needs a change in wgpu's
  API layer (passing the objects instead of their ids).
- **Bind groups made per frame** by the grass culling and drawing (each
  batch's buffer paired anew with a pool's buffer): ~150 a frame. Storage
  buffers in groups of their own removed them but made encoding slower,
  with no gain in frame rate; kept aside.
- **SRV slot resolution** (texture formats looked up per draw): 8 % of the
  command thread; could be computed once per view.
- **BC texture decoding on Android** (re-encoded as ETC2): two threads at
  ~17 % each while the world streams. Off the critical path, but it costs
  battery and heat.

## CPU side: the engine

- The engine's threads (main, render, workers, audio) are now the busiest:
  83-97 % of their time is the game's code. ThinLTO of that code
  (`GLUE5_GAME_LTO=1`) showed no gain, on a Mac or on the Pixel. PGO is
  possible on a phone now (`mise run pgo:android`, see
  [the S21+ document](docs/android-performance-s21.md)), but still
  untried there: the profile has to be taken on the device, and a build
  with it measured against one without. The profile itself is written by
  the app (`__llvm_profile_write_file`), not by the exit handler: the
  process ends without one (`glue5_runtime::exit::process` ends it with
  `_exit`, the engine's threads still running).

## Measured and rejected

- ThinLTO of the game's code: no gain (see above).
- Storage buffers in bind groups of their own: fewer bind groups, slower
  encoding, same frame rate.
- Arm's Accuracy Super Resolution: the library is C++ with GLSL and HLSL
  shaders (no WGSL), so it would need a full port, and it is derived from
  FSR 2.2.2 while this tree already ships FSR 2.3.
- Swappy: it paces Vulkan and OpenGL swapchains; an Android game's
  pacing is `Choreographer`'s and `ANativeWindow_setFrameRate`'s.
- `StoreOp::Discard` on the renderer's own targets: every attachment the
  engine draws into is read again later (the scene's colour by the
  compose pass, the depth by the motion vectors, the mip chain by the
  next level), and a discarded attachment is not in memory to be sampled.
  The last image of the frame is the compose pass's input, so it cannot
  be discarded either.
