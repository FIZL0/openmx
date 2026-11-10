# 1 — s13 laps (FINAL)

Task S292a. 2026-11-09. The rest of the score manager's mode-1 handler (the machine race,
`./kit fn 0x2640c0`) around S292's gate line and zone list.

## What I did

- `crates/game-sim/src/race.rs`, a new section at the end:
  - `LapRules::machine_tick`: the whole handler. It returns a `MachineTick` with `went_on` (the
    handler's non-zero answer), the per-card `MachineNotice`s and the close's `FinishNotice`s.
  - `LapRules::machine_card`: one card's turn through the loop. The private `machine_step` does a
    racing card's part.
  - `LapRules::machine_close`: what runs after the loop.
  - `LapRules::gate_rank`: the gate race's ranking.
  - `MachineNotice` (Look, Line, Zone, Aimed, Aim, TimeUp), `MachineTick` and `PATH_STEER`.
- The machine card is a `RaceCard`. `to_track` holds +0x180 (the gate aim), `look` holds +0x120, and
  `odometer` holds +0x20.
- `LapRules::rank`'s body moved, unchanged, into a private `rank_by(cards, ahead)`. `rank` and the new
  `gate_rank` differ only in who counts as ahead. `rank` and `gaps` became generic over
  `AsRef<LapCard> + AsMut<LapCard>`, so they take `LapCard`s and `RaceCard`s alike. The callers are
  unchanged. I chose this over copying 60 lines of sort-and-notify.
- `research/oracle.txt`: the full handler in words under `score_machine_race_tick`, and a new
  `score_gate_rank` entry.
- `crates/game-sim/tests/race.rs`: items 25 and 26 (below).
- Filed **S292c** (low) for the two branches I did not port (see "Not verified").

## What the original does

The handler gets dt in f12 and the manager in a0. It answers 0 when the manager has no cards (+0x34)
or no gates (+0x70). Otherwise it runs the over check (the same as `check_over`), then each card in
list order:

1. Owner and chain vt+8 (card).
2. A card that is not racing (no flag 2, kind 4 without 0x100, or any of 0x4001): if it has 0x4001,
   owner and chain vt+0x20 (card+0x60, card+0x120).
3. A racing card:
   - The odometer +0x20 grows by |now − last| (VU vsub.xyz, then mula/madda/madd, sqrt.s, add.s).
   - Flag 0x80000 calls a reset helper (`./kit fn 0x267d90`).
   - Then, only while `!(last.x < 0)` (c.olt.s, so NaN and −0 still run it), the gate line runs. A 0
     from the gate line makes the whole handler return 0 at once, with no further cards and no close.
     After it: a card out with 4 or 0x8000 sets mgr |= 4, then the zones run if mgr has 0x200.
   - The distance from the gate centre to the last spot goes to +0x10, and to the current spot to +0x14.
   - If +0x10 ≤ 50 (c.ole.s), the aim is gate + (next − gate)·(0.02·(50 − +0x10)) − now. Here `next`
     is the gate's +0x90, or the first gate after the last one. Otherwise the aim is gate − now. The
     aim is then made a unit vector with vrsqrt and stored at +0x180.
   - Listener vt+0x6c (with its chain), then owner and chain vt+0x5c.
   - Progress: +0x1c = +0x24 − +0x14. Then last = now (xyz only).
   - If the race is over and the card is not out, the finish is made up. There is no kind test here,
     unlike `score_race_clocks`:
     - +0xb4 = the head card's +0xb4 + this card's gap.
     - +0x48 = (+0xb4 − +0x4c)/(laps − +0x30) when laps remain, else the bare difference.
     - Then the best-lap clamp, laps = race laps, and |= 1.
   - Otherwise the race clock and lap clock run.
   - Kind 4 with 0x200: the attack clock.
4. Every card: the time limit, the same as `time_limit`.
5. Every card: if the manager has no track (+0x68) or no 0x800000, owner and chain vt+0x20
   (card+0x60, card+0x180). Otherwise a path-steer branch.

After the loop it runs `score_race_close`'s code with two differences: the countdown has no kind test,
and the ranking is `score_gate_rank` (`./kit fn 0x262470`). That ranking is score_rank's sort with a
different test for ahead: q is ahead of p when it has passed more gates (+0x38), or the same number and
its +0x14 is smaller (c.olt.s). It does not test the track.

## How it was proved

Function oracles in the Ee interpreter, every word compared bit for bit, in `crates/game-sim/tests/race.rs`:

- **25** `the_machine_race_matches_the_score_managers_mode_1_handler`: `score_machine_race_tick`
  against `machine_tick`.
  - 20,000 generated managers, each with 1–5 cards, chains of 1–4 gates and zone lists. The inputs
    vary all card and manager flags it reads, the kind, the laps, ghost cards, free records, a dt that
    is sometimes stray, riders near and far from their gate, and negative last x.
  - Compared:
    - every listener call in order, with its card, zone, other card and the two quads a +0x20 call
      points at
    - the answer
    - the list order after ranking
    - every card's words, including gate words, odometer, look and aim
    - every ghost card's words
    - the manager's flags, the countdown, the free-record pool and the rand state
  - Coverage floors: every notice kind over 100 times, except RunOver and GhostReady (over 40). At
    least 200 handlers stopped by the gate line, more than 2,000 cards each at ≤ 50 ft and > 50 ft, and
    more than 1,000 over-race made-up finishes.
- **26** `the_gate_ranking_matches_the_score_managers`: `score_gate_rank` alone, on 20,000 lists of 1–6
  cards with ties in gates passed and in distance, against `gate_rank`.
- Mutant checks, each failing as it should and then restored:
  - the aim lead 0.02 → 0.021 failed 25
  - the tie test `lt` → `le` failed 26
- `./kit check -- -p game-sim --test race -- machine gate_ranking`: 3 passed.

## Not verified / not 1:1

- **The card flag 0x80000 helper** (`./kit fn 0x267d90`, 1340 bytes, a reset that copies the card's
  spot and tells owner, chain and listener vt+0x90/+0x88). Not ported. `machine_step` skips it, and the
  generator never sets 0x80000. Filed as S292c.
- **The path-steer branch** (the manager has a track and flag 0x800000). Not ported because it needs
  the path spot lookup. `machine_card` always takes the owner vt+0x20 branch, and the oracle's manager
  has no track. Filed as S292c.
- **A card waiting at no gate whose last x is negative.** The original skips the gate line and then
  reads the gate centre through a null pointer (low memory). The port returns `false` (stops) instead.
  The generator never makes this case, so it is unproven and deliberately not 1:1. A card with no gate
  and last x ≥ 0 goes through the gate line, which answers 0, and that path is proved.
- **The handler's non-zero answer.** It is what the close leaves in v0. The port reduces it to
  `went_on` (0 or 1). The oracle checks it only as `v0 == went_on as u32`, and that passed over all
  20,000 cases, so in practice the answer is 1. No other value is modelled.
- **No live capture of a machine race.** Everything is proved per function in the interpreter. Nothing
  calls `machine_tick` from the Sim yet, the same as S292's `gate_line`.

## Next

`./kit done` ran with `--skip-audit`. The audit's one leak is S294a's plan line, which names the path
distance's address. That line was already on main (73b7cce2) before this task, and this task did not
touch it. The audit reports no leak from the files this task changed.


Land it. S292c holds the two unported branches. S292b (the AI rider's gate steering) is unchanged.
