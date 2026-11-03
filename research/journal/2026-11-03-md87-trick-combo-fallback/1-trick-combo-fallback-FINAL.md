# 1 — the combo falls back (FINAL)

Task MD87. 2026-11-03.

## What was wrong

MD77 gave every pairing of Cross/Square with the original's three buttons its own group
(`trick::group_with`): R1 + Cross is group 16, R1 + Square 23, R1 + both 30, Cross + Circle 10, and so
on. The packs fill the extra slots in order, starting at group 7, so a pack of up to 36 stunts fills
groups 7..=15. That leaves R1 + Cross and R1 + Square empty. `Tricks::request` dropped a gesture whose
extra slot was empty, so holding R1 (the original's modifier, which a rider holds for the front flip
and the 360s) turned Cross and Square off.

## The pick (`game_sim::trick::{fallback_groups, pick}`)

`fallback_groups(group)` lists the groups a gesture tries, in order. `pick(set, group, dir)` starts the
first one whose slot at the stick's direction holds a trick. The 24 always count as filled, and an
extra slot counts when the stacked packs reach it. `Tricks::request` calls `pick` where it used to
call `trick_of` with an emptiness filter.

The order:

1. **The combination itself.** A combo overrides only when there is a new trick in it.
2. **R1 dropped**, if it is held. R1 is the original's own modifier, never a group by itself, so it
   goes first: Cross + R1 → Cross (7), Square + R1 → Square (8), both + R1 → both (9), Cross + Circle +
   R1 → Cross + Circle (10).
3. **The extras dropped**, leaving the original's gesture for the original's buttons as they are held:
   Cross + Circle → Circle (1), Cross + Circle + R1 → Circle + R1 (4). That slot is always filled, so the
   chain ends there.
4. When none of the original's groups is under the extras and both are held: **Cross alone (7), then
   Square alone (8).**

**Why the order is this one.** The brief left "Circle's or Cross's" for Cross + Circle to the
original's order. In the original, a combination always beats the buttons inside it (`trick::group`
tests all three, then the pairs, then the singles). It also treats R1 as the modifier the other
buttons sit under. MD77's numbering puts the extras *over* the original's groups the same way. So the
extras are the modifier over Circle and Triangle, and dropping them leaves the original's trick: Cross +
Circle → Circle. Cross over Square, for both alone, is a choice of ours. The original has no precedent
for it, because none of its pairs is ever empty. We take Cross first because the button order
everywhere else (`EXTRA_BUTTONS`, `extra_group_name`) names Cross first.

`fallback_groups` of 0..=6 is just the group, and a rider without a pack never has its extras held
(`rider.rs` clears them), so the original's tricks, the AI and every replay are unchanged.

## The Tricks page (P259)

The page lists every trick by its own gesture: each filled slot once, under its own buttons. A combo
that falls back starts a trick that is already listed under its shorter gesture, and the page never
lists an empty combo as doing anything. So the page already shows what each gesture does after the
fallback, and no change was needed there.

## Checked

- `cargo test --release -p game-sim --lib trick::`: the new `combos_fall_back_without_their_modifier`
  covers the chains. All 10 pass.
- `cargo test --release -p game-sim --test trick`: the new
  `r1_over_an_extra_falls_back_to_the_extra_alone` passes. With R1 held, Cross and Square at all four
  directions start Cross's/Square's trick when the R1 combo is empty and the combo's own when it is
  filled. Both + R1 falls to both. Cross + Circle falls to Circle. Groups 1..=6 with the extras up are
  unchanged, and R1 alone still starts nothing. 11 of 12 pass. `the_humans_tricks` fails on this slot's
  C6 capture ("rows whose pad is not field 2k - 2"), which is a known, unrelated failure. That test
  never holds the extras.

## Not verified / not 1:1

- Not ridden in the shell on a pad. The fallback was checked in game-sim only.
- The fallback order is the port's own: the original has no empty slots. Cross before Square for
  both-extras is a choice, as described above.
