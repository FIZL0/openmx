# 1 — texture reload (FINAL)

Task V86. 2026-10-26.

## What I did

`OPENMX_TEXTURE_RELOAD=1` (any value but empty or `0`) makes a key-named file that is new or rewritten while
a ride runs be drawn from the next frame, in both renderers.

Taken from opentennis (`crates/hst-base/src/textures.rs`, B83's `reload`):

- The folders are walked once a second, on a thread of their own. opentennis uses Bevy's IO pool; here it is a
  `std::thread` whose handle is checked with `is_finished`, so the ride's frame never waits on the walk.
- A key whose winning file changed is read again. opentennis compares path and mtime; here it is path, mtime
  and length.
- It is off by default, because a walk a second stutters on slow storage (B83's reason).
- A file taken away keeps its picture until the next ride, as opentennis's does (its "ponytail" note).
- The log line is `textures: reloaded <file>`.

What changed for openmx, where there is no Bevy `Assets<Image>`:

- **Each loaded texture remembers its key.**
  - `replace::swap_keyed` returns the disc key alongside the (maybe replaced) texture.
  - `world::decode_texture_keyed` uses it, and `Assets` keeps a private `keys` map from texture index to key,
    filled in `resolve`.
  - `take_in` (a world's tiles) carries the keys across, and `let_go` drops them.
  - `Assets::replace_keyed(key, &Texture)` swaps every texture of that key and returns their indices.
  - The software renderer samples `assets.textures` directly, so for it the swap is the whole reload.
- **GPU**: `Renderer::reload_textures(&assets, &indices)` drops those indices' uploaded views (every mip
  cutoff) and rebuilds each material that draws one, in its place. Building a material was split out of
  `material()` as `make_material`, and `GpuMaterial` keeps its `Surface` so it can be made again. Models keep
  their material indices, so nothing else moves. The levels are rebuilt by `upload_texture_keeping` as on
  first upload.
- **The source** that `install` sets now reads a shared `Arc<RwLock<HashMap>>` that the reload updates, so a
  texture decoded after a change (a world tile streaming in) gets the new file as well. With the variable set,
  a source is installed even over empty folders. Otherwise nothing would be keyed, and a file put in after
  the ride loaded would find nothing.
- **The shell**: `install_textures` returns the `Reload`, and the ride loop polls it at the top of each frame.
- The per-second walk does not repeat the "pack has no textures" log line (`packs_saying`).
- `textures::install` keeps its signature (the V84 tests call it); `install_live` is the new entry point.

## What the original does

Nothing to port: this is presentation, a texture author's tool on top of V84's departure.

## How it was proved

`crates/openmx/tests/texture_reload.rs`:

- **`a_rewritten_file_reaches_every_texture_of_its_key`** (no disc). It uses a hand-made `.pst` loaded through
  `Assets::resolve` under a live source.
  - The texture keeps its key.
  - A file put in after load, a rewrite at another size, half a file (not taken until it is whole) and a broken
    picture each behave as described.
  - A texture decoded after the change gets the new picture.
- **`a_file_rewritten_mid_ride_is_drawn_with_the_variable_set`** (disc). The setup:
  - Every Bike\ and Rider\ archive texture (over 300 keys) is replaced by a 4×4 magenta PNG.
  - The shell rides free06 headless at 320×224, 300 frames, and is shot at frame 299.
  - When the shell prints `headless, drawing`, the test rewrites every file green, atomically.

  The results, for `--renderer gpu` and `--renderer software` alike:

  | renderer | variable | magenta px | green px |
  |----------|----------|-----------:|---------:|
  | gpu      | unset    | 717        | 0        |
  | gpu      | set      | 0          | 714      |
  | software | unset    | 720        | 0        |
  | software | set      | 0          | 713      |

  Without the variable there are no `reloaded` lines. Each ride takes about 5 s plus loading; the test takes
  about 48 s in a debug build.

## Not verified / not 1:1

- **Only the models' textures reload**: props, bikes, riders, the biplane and the other machines, and a world
  tile's. These are read at ride load and are not reloaded mid-ride; a change to them shows from the next
  ride:
  - the ground's splat layers, detail map and painted road;
  - a mod map's terrain pages, which share one texture array and `swap_all`'s resample;
  - a world's baked pages;
  - the start zones (`portal.rs`) and the tire tread.

  Each has its own upload path in `gpu.rs` (`upload_pages`, `upload_array`, `upload_world_pages`) and its own
  copy of the pixels on the software side. Re-baking them per key was out of the brief's "one texture (and
  its mips) by key". This is written in docs/TEXTURES.md.
- **A file taken away** leaves the replaced picture until the next ride. The disc's pixels are not kept, as in
  opentennis.
- **A texture first decoded while no source was set** has no key and never reloads. That covers the made
  textures too (a rider's painted name and number, P86).
- **The ride test's timing** depends on the shell pacing headless rides at 60 fps. The rewrite happens at the
  first drawn frame, and reload has the remaining ~5 s of frames to land (one walk per second). A shell that
  ran headless frames unpaced and fast could finish before the walk. This was not seen here.
- **Frame cost of a reload** (PNG decode on the ride thread, the re-upload and its levels) was not measured
  with 4× textures. The walk itself is off the ride thread.
- **The front end's menus** do not poll: a reload runs only inside a ride.

## Next

Nothing: done.
