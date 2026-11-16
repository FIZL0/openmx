# 1 — quad looks online (FINAL)

Task MD171. 2026-11-16.

## What I did

- `crates/openmx/src/main.rs`: the ride holds a list of quad looks keyed by pick (`pack/stem`,
  `quad_key`) instead of one. This machine's own is loaded at the ride's start as before; another
  seat's is loaded (`seat_quad_look`, off the room's files or the mods folder, as `quad_reader`
  reads its handling) the first time that seat's quad shows, and its models go up to the GPU then.
  A pick whose look fails is not tried again.
- Every draw of a quad (GPU, software, the rider on it) looks up the seat's own look
  (`quad_look_of`).
- The bug under the task: other seats' garages are dressed as copies of this machine's, quad
  included, so MD152's "take the seat's quad when the garage has none" never ran and every seat was
  drawn as this machine's quad (its look, and its stem in the drawn copy; the session's lanes
  always stepped each seat's own quad). Now a garage whose quad is not the seat's pick takes the
  seat's pick (with its look's stunt table).
- Clips online (`tick_lane_clips`): after each seat's state is shown, its quad clips tick once per
  session tick run since the last drawn frame, the last on the shown tick's calls, the ones before
  on none; made on the look's stunt table when the quad first shows. Offline is unchanged (its
  step still ticks them).
- The ride's end prints `online: quads drawn 1P <sub>, clips <s> s, played; 2P …`.
- Took nothing from opentennis: it has no per-seat vehicle models.

## What the original does

Nothing to port: online play is ours, and how a quad is drawn is presentation.

## How it was proved

- `crates/openmx/tests/online_atv.rs` (two headless online shells, `--peer`): the guest now picks
  the sport quad `atv_s_Pro250A`. Both machines must print 1P drawn in an `ATV_R_` build and 2P in
  an `ATV_S_` build, each with clips ticked over a second and playing; MD152's checks (no desync,
  the same state hash) still hold with the two different quads.
- By hand, the same two shells: `2P Veh/Anim/ATV_S_450F_KFX4001.sub, clips 10.83 s, played` on
  the host, which drew 2P in its own `ATV_R_250_250RTHQ1.sub` before the fix.

## Not verified / not 1:1

- Nothing looked at on screen: the test runs headless and reads the log line. The rider on another
  seat's quad uses that look's rider clips through the same `quad_draw::rider` as offline, but no
  picture of it was taken.
- Clips online are an approximation of offline's: a drawn frame that shows several new ticks plays
  only the last tick's stunt calls (the shown state holds only those), so a stunt started on a
  skipped tick is missed; a shown prediction later rolled back can start a clip the settled ride
  never had. Catch-up is capped at 8 ticks a frame. Presentation only, never in the state hash.
- Looks are keyed in `Assets` by chassis name (`quad:<chassis>`, as MD138 named them): two
  different quad packs with a chassis of the same name would share one model. Only one quad pack
  exists today, so this was not tried.
- Seat reshuffles (a join or a leave) keep each shell seat's last-ticked count by shell index, so
  the first frame after one may tick a seat's clips by the wrong number of ticks.

## Next

none
