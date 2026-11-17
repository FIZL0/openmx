# 1 — give back ground (FINAL)

Task P307. 2026-11-16.

## Why it went ahead

The task waits on a phone log after P298a. Three are in `~/Downloads` (`phone-114109-…`,
`phone-115536-…`, `phone-160542-openmx-log.txt`, 2026-11-13, build 4179). All of them ride world_mxu,
a streamed world. Each DidEnterBackground still shows the ride high just after
`away: gave back … models`:

- `phone-160542`: 1313 models given back, then `answered after 115 ms, memory 1311 MB`. Just before
  going away the ride was at about 1350 MB with GPU 680 MB. The next frame line says 1300 MB with
  GPU 621 MB. So the models were only ~60 MB of the GPU's share.
- The same pattern shows in the other two logs: 1.06–1.27 GB in the background.

So the condition holds, and the ground goes next, as P298a's journal says.

## What I did

- `Renderer::give_back_ground` (new, `crates/game-render/src/gpu.rs`). It runs in the background and
  submits no GPU work. It does three things:
  - It drops every ground piece except the stand-in (V92).
  - It drops the vertex pages after the last page the stand-in's chunks are on. Phones use lean
    32 MB pages (P301a). The stand-in goes up first at the load, so it sits on page 0, and every
    later page goes.
  - It frees the paint's page arrays. These are fixed pools: world_mxu's are 128×128 × 1440 layers
    and × 592 layers, about 175 MB with mips. Giving back each map's layers only frees slots, so
    each array is swapped for a 1-layer stand-in array of the same format and the ground's bind
    group is made again. Every map goes back to its far pages, so the leaves never read the small
    array.
  - It returns pieces, chunks and bytes.
- `Renderer::take_back_ground_paint` (new). It makes the arrays again at their own size (layers from
  the pool, as `upload_world_paged` does) and rebinds them. It does nothing if they were never
  given back. It must run before a map's pages are written: a write past layer 0 of the small array
  would be a wgpu validation error.
- `GroundStream::given_back` (new, `worlds.rs`). It lets go the parts not yet handed over, then runs
  MD97's `build_again`. Without this, a piece half handed over would get its `More` parts after the
  GPU forgot its first part.
- `PaintStream::given_back` (new, `paint_stream.rs`). Shown and Writing maps go back to Far and the
  writing queue is cleared. Maps still being read stay Asked and are written when they arrive.
- The ride loop (`main.rs`, P298a's away block). After `give_back_models`, if the ride has a ground
  stream, it calls the three above and logs
  `away: gave back N ground pieces (C chunks), M maps' pages, X MB, in the background; K pieces to build again`.
  This is before `away::answered`, so DidEnterBackground's `answered after … memory …` line shows
  the memory after it.
- On return, MD97's existing "back in front" block (in front again, `times_away` moved) now also
  calls `take_back_ground_paint` and logs `world paint: page arrays made again at tick T`. This
  happens before `PaintStream::update` that frame. The ground stream rebuilds every held piece. The
  stand-in is drawn under each piece until it comes back, so the ground is coarse for a moment under
  the pause rather than bare.
- opentennis: nothing to take. Its iOS code frees nothing in the background (as P298a found).
- Changed (and why): the task said "bare a moment". With the stand-in kept, it is coarse, not bare.
  The stand-in's 272 chunks sit on page 0 with the first pieces, so keeping it costs one lean page.

## What the original does

Nothing to port: the PS2 game has no suspend.

## How it was proved

- `crates/openmx/tests/ground_give_back.rs` (headless GPU, lean pages on). The setup is a two-map
  world (an array each), a stand-in, a piece in view and a 400-chunk piece off view, which forces a
  second vertex page.
  - `give_back_ground` gives back 2 pieces and 401 chunks, 13016 KB here.
  - The ground keeps 1 chunk on one 330-chunk page.
  - The paint pool has no layer taken and no map shown.
  - A frame drawn in that state works, and differs from before on >5000 pixels.
  - A second give-back returns (0, 0, 0).
  - `take_back_ground_paint` is true once, then false.
  - After the pieces are added again and the maps' pages written again (as the paint stream does),
    the frame is **byte-identical** to the frame before. A temporary panic confirmed the test really
    ran on an adapter (the `gave back … 13016 KB` line in the check log).
- `worlds::tests::the_ground_given_back_comes_again_whole_not_from_half_a_piece`. It hands pieces
  over a chunk a frame and gives back with parts still pending. The pending parts go. Every piece
  then comes again first part first, with the same corners bit for bit as the first build.
- The MD97 rebuild test still passes.

## Not verified / not 1:1

- **Not run on an iPhone.** There is no device here. Unchecked: how far the background footprint
  actually drops, that freeing the arrays and pages returns memory to iOS, and that the app now
  outlives a long suspension. The phone's log will say: the new `away: gave back … ground pieces …
  MB` line, then `answered after … memory … MB`, against the ~1.3 GB the 2026-11-13 logs show.
- **The iOS target was not built** (`cargo check --target aarch64-apple-ios`). The new code has no
  `cfg` and builds in the host's openmx binary. The iOS-only calls it uses (`away::backgrounds`) are
  P298a's and unchanged.
- **Not measured: the hitch on return.** The first frames back read every near map's pages from its
  folder again (P283's worker thread) and rebuild every held piece (MD97's worker). Both are off
  the main thread, but writing the pages and pieces in is a frame budget's worth at a time. The
  ground shows the stand-in, and far paint, for longer than MD97's rebuild alone did. All of this
  happens under P298's pause.
- **Still held in the background, by design or not measured:** the stand-in and its vertex page 0
  (a lean page, ≤32 MB), the far atlas, the details array, the splat layers, the leaf image, the
  shadow maps, the upscale targets, the dressing's prop instances. On the CPU: assets, the world's
  tiles and the sim. If the next phone log is still high, those are next. wgpu reports no byte
  counts, so the split of the remaining GPU share is unknown.
- **The vertex page trim relies on the stand-in sitting on the first pages.** That holds when the
  stand-in goes up before any piece, as the load does it. If a stand-in were ever set after pieces,
  its chunks could sit on a late page, and fewer pages would be freed. That is not wrong, only less.
- **Android** runs the same path (its `away::backgrounds` counts activity stops, P311). Not run
  there either.

## Next

Nothing for this task. The next phone log from a world ride says whether the background footprint
dropped (see above).
