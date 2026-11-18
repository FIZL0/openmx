# 1 — prop rays meet props in walk order (FINAL)

Task S385. 2026-11-17.

## What I did

- A prop's rays on the other props (`prop_props.rs` `strikes_of`) still run first in flat order. When
  they meet two or more props, the port now asks for the order the striker's gather walks the static
  tree in, and runs the rays again in that order. Props the walk does not list go after, in flat order.
- The order comes from `flight_search::knocks_walked`: the bike's gather world (`gather_knocked`'s, F8 /
  F68) with every other prop's `f.col` filed where it stands, gathered afresh round where the striker's
  kept list was gathered (or its rays' centre if it has none). Only the knocks' places in the list are
  read.
- `strike_props` / `strike_props_late` now take a `Pass` (a struck prop's own search, plus the walk).
  Two kinds: `Among` (scene, plants, terrain fixed: `PropSet::search`, `search_among`) and `InWorld`
  (`search_in_among`, `strike_in`: the world near the striker). Small edits in `prop_set.rs` and
  `prop_contact.rs` for the call sites; that is outside the brief's file list, but additive.
- The walk is only asked when two or more props are met, so the usual tick builds no extra world.
- Test: `tests/prop_plane.rs` `a_ball_the_plane_wakes_strikes_the_ball_beside_it` now runs gaps 4.8
  and 4.4 ft too.
- Took nothing from ../opentennis (it has no props-on-props search).

## What the original does

From B34b and F68 (no new reading): a prop's ray probe meets the other props through its kept nearby
list, which is gathered in the static tree. Every prop's `f.col` stays in that tree asleep or awake,
keyed where it stands. So the props come in the order the query's walk reaches the tree's cells, the
same as in the bike's gather (`tests/prop_wheel_gather.rs`).

## How it was proved

`a_ball_the_plane_wakes_strikes_the_ball_beside_it`, the interpreter scene from B34b (b34at-11065, ball
1 put asleep by the original's object reset). At 4.8 and 4.4 ft ball 1's rays meet ball 0 and ball 2.
The original's list has ball 2's entries before ball 0's. Before this change the port had them the other
way round (seen at the start of this task: 7 and 7 entries wrong). Now ball 0's and ball 1's whole lists
(17 and 14 entries on ball 1), flags, wake state and the filed order match bit for bit at all six gaps
(6.4 … 4.4). The walk returned the flat places 3, 2, 5, 4, 0, so ball 2 (flat 2) before ball 0.

Also green: `prop_plane` (8), `prop_props` (5), `prop_wreck` (7), `knock_launch` (5),
`prop_wheel_gather` (1).

## Not verified / not 1:1

- **The test's world is terrain only.** It has no scene meshes or plants, so the walk there is over a
  tree with only the props in it. I argue it is the same order as the full scene's: the tree is laid
  over the terrain's box either way (`laid_box`), the props are filed after the scene and the plants,
  and a node lists its slots in filing order, so other items don't reorder the props. Not checked with
  free06's scene meshes loaded.
- **The kept list's contents are not kept.** The original meets only the props on the striker's kept
  list. The port still meets every prop; the ones the walk does not list go last, in flat order. No case
  where a prop is met but not listed was seen.
- **Where the list was gathered.** The walk is gathered afresh at the striker's kept list's centre (or
  its rays' centre). A list the original gathered ticks ago, before a prop moved into or out of the
  queried cells, can name other props. Not compared.
- **Scene mesh and prop in one pass:** still as before, a prop's entries go after its scene's (the older
  gap in prop_props.rs's docs).
- **Bike lane and the `search` / `search_among` paths:** only the plane lane (`strike_in`) was compared
  against the original. The others use the same code with their own world.

## Next

None: the task lands.
