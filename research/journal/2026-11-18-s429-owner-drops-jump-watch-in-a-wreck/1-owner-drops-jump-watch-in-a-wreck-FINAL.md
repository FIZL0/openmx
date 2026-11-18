# 1 — owner drops jump watch in a wreck (FINAL)

Task S429. 2026-11-18.

## What I did

- New `RaceCard::riding_bit` (crates/game-sim/src/race.rs): a down rider's card loses
  `WATCHES_JUMPS`; a rider who is up has it set.
- `Laps::tick` calls it for each card just before `RaceCard::feed`.
- `RaceCard::feed` is unchanged. It stays bit-exact to the original's bike-to-card word, which never
  touches 0x80000 (the S302a oracle).
- tests/ai/race_sx01.rs: `OWNER_FLAGS` is now only the listener bits `0xe0`. The jump watch is
  compared as the Sim makes it.
- research/oracle.txt: where the ride state machine clears the bit.
- Nothing taken from ../opentennis (no race cards there).

## What the original does

- The ride state machine (`./kit dis 0x0027bc90`) ends each tick with: if the wreck reason
  (bike +0x748) is non-zero and the race record (+0xf9c) exists, clear bit 0x80000 of its flags
  (`./kit dis 0x0027be2c`, 0x27be2c–0x27be50). This runs after the reset request, so a same-tick
  respawn leaves the bit alone.
- The respawn (`./kit dis 0x0027bb20`) ORs 0x80000 back in, unconditionally.
- The wreck entry (`./kit dis 0x0027b810`, `./kit dis 0x0027b6e0`) does not touch it.
- The speed word freezes on its own: the bike-to-card word still writes |chassis velocity| each
  tick, but the base vehicle update skips physics while +0x748 ≠ 0, so the velocity stands.
- The Sim's `state::Step::riding_bit` already ports the same rule; nothing consumed it.
  `riding_bit` reads the same thing off the feed's `wrecked` (`state.wreck.is_some()` after the
  rider's frame), since the race frame runs after every bike.

## How it was proved

- C31 rider 1, frames 5915–6021 (context/capture/c31/sx01-ai4-by-rider.csv): bit gone from 5915,
  when the velocity freezes; back at 6021 with the respawn's zero velocity.
- `s377_the_sims_race_riders_hold_c31s_live_sx01_race` now compares the card flags with the jump
  watch included, over C31's whole live race (S377's resynced and free runs): passes.
- Mutant: with the `riding_bit` call taken out, the same test fails on `flags`. So the bit is
  really compared, and the test did not skip.
- `the_bikes_word_to_its_card_matches_the_original` (20,000 oracle cases) still passes:
  the feed is unchanged.

## Not verified / not 1:1

- **The C31 check is circular for the bit.** C31 holds no bike wreck word, so the test still marks a
  card wrecked when the captured card lacks the bit. The test proves the Sim's handling of the bit
  (cleared while down, back on at the respawn, jumps watched in between), not the wreck timing.
  The Sim's own wreck timing is proved elsewhere (S5's state machine).
- **Set on every tick up, not only at a respawn.** The original sets the bit only at a respawn, and
  keeps whatever the card holds otherwise. The port sets it on every tick the rider is up, to add no
  new saved state (a field would grow the netplay wire and snapshot). Same result for every card the
  Sim makes, since `HUMAN_CARD` and `COMPUTER_CARD` both carry the bit. It would differ for a card
  made without the bit: the original gives it the watch only at its first respawn.
- **Guest seats' feeds** (P350, netplay) carry `wrecked` as their holder sends it; not checked here.
- **The speed word freeze in the Sim** rests on the Sim not stepping a down rider's chassis
  velocity; not checked against a capture in this task.

## Next

Nothing: done.
