# 1 — d7 freeride bike class (FINAL)

Task D7. 2026-10-15.

## What I did

A freeride bike is a **125, Race tier** — not the 250 the port assumed. `FREERIDE_ENGINE` is now
`Engine::Cc125`, with a new `FREERIDE_PRO = false` beside it for the tier, so the pair the physics proofs
(S3, S10, S11) are compared against is `Bike\Race125P.phy` and `Bike\Race125A.phy`.

Three things were wrong in the old comment, and all three had the same cause — reading a directory listing
instead of the loader:

1. the class (250 → 125),
2. the claim that "the freeride archive ships only the 250 pair of each tier". All sixteen names are on the
   disc. The bike `.phy` pairs ship inside the bike **model** archives (`Bike\yz125.res`,
   `Bike\crf250_faisst.res`, …), each carrying its own class's four files; the *track*'s second archive
   (`Tracks\free06_2.res`) carries the 250 pair, the wreck bike and every other vehicle's physics, which is
   the listing that produced the wrong reading,
3. the spelling: the loader's prefix is `Bike\`, capitalised, not `bike\`.

## What the original does

The vehicle build loader — `./kit fn 0x00208c50`, and `./kit dis 0x00208e40 0x00208f40` for the part that
matters — reads the event (`*( *(app+0x3d4) + 0xc )`, app at 0x00479990) exactly twice for this:

- **event+0x110, the tier**: `0` → the `Race…` names, anything else → the `Pro…` names;
- **event+0x10c, the class**: `0/1/2/3` → `50/125/250/500`.

From those two it `snprintf`s **two** names into two 0x104-byte buffers, the `…P.phy` of the pair first and
the `…A.phy` second, out of the sixteen format strings at 0x0042e0d8…0x0042e1c8 (`"%sRace50P.phy"` …
`"%sPro500A.phy"`; P before A inside a class, the eight `Race` before the eight `Pro`). The `%s` is the
prefix at 0x004e2000, which `./kit fn 0x00200df0` fills once at startup with `Bike\` (0x0042d838) —
alongside `Rider\`, `Biplane\` and `helicopter\` for the other vehicles. So the name the loader hands the
resource system is literally `Bike\Race125P.phy`. The archives spell their members lowercased
(`bike\race125a.phy`), so the lookup folds case somewhere I did not chase.

Nothing in that function makes the choice mode-dependent: the activity-type-5 branch above the switch loads
`machine.res` and never touches the two fields.

**So "which bike does freeride load" is "what is in those two fields when a freeride world loads".** They
come from the event's constructor, `./kit fn 0x002b0780` (0x148 bytes, new'd by `./kit fn 0x002b20c0`,
MXSettings' init, which also sets league 0 / type 2 and the first track record's name):

| field | ctor | meaning |
| --- | --- | --- |
| +0x10c | **1** | 125cc |
| +0x110 | **0** | Race |
| +0x114 | 1 | players |
| +0x115 | 5 | laps |
| +0x117 | 0x5a | AI difficulty |
| +0x11c/+0x120/+0x124 | 6 / 0 / 4 | freestyle course record |

Only two things in the image write the bike fields, and neither fires on a retail freeride boot:

- `./kit fn 0x00203260` (`MXGame::Configure`) reads the `.cfg` keys `BikeClass` (`"50"/"125"/"250"/"500"`
  → 0/1/2/3, **default `"250"`**) and `PhysicsModel` (`"Race"` → 0, `"Pro"` → 1, default `"Race"`) — both
  gated on a section the retail disc does not ship (R7 found the same of every key). Those two defaults are
  exactly the port's old guess, which is how a 250 got written down;
- `./kit fn 0x00203dc0` forces the class to 2 straight after building the settings object, but only when
  the word at 0x004e2240 is 2. That word is in `.bss`, reads 0 in both RAM images below, and all 38
  references to it in the dump are compares — not one is a store. It is a build/kiosk switch, not
  something a retail session reaches.

The names are in `research/oracle.txt` under `# D7`.

## How it was proved

Two RAM images and the disc. No oracle run: this is a choice of a name, not arithmetic.

**1. The human's own freeride recording** (`context/recordings/first_recording.p2m2_SaveState.p2s`, read
with `./kit state extract`; the EE image is in `context/states/d7/`). Settings at 0x0050d8f0, event at
0x00506580: league 2 (free), type 5 (freestyle), track `Free09`, **class 1, tier 0**, 1 player, 5 laps, AI
0x5a — the whole constructor block still at its defaults. The prefix global reads `Bike\`. The course
record fields are 4/1/0 against the ctor's 6/0/4, so that entry path *did* write part of the event; it left
the bike fields alone. The only bike physics names resolved anywhere in its 32 MiB are
`Bike\Race125A.phy` and `Bike\Race125P.phy` — four instances, each followed 260 bytes (one buffer) later by
`Bike\Wreckbike.phy`. No 250 and no `Pro` name is resolved anywhere in the image.

**2. A scripted freeride boot** (PCSX2 copy 3, one `./kit pcsx2 with --` job each, scripts in the
session's scratchpad). R7's recipe *minus* its two bike writes: poll until a scene exists while writing
`app+0x408 = 2`, `event+0 = 2`, `event+4 = 5`, `event+8 = "Free06"`, `NumberOfRacers = 1`,
`event+0x114 = 1`, `AllAI = 0`, and deliberately never touching +0x10c or +0x110. Result: scene at
0x004fdfb0, `in_game = 1`, frame counter advancing ~30/s, kiosk word 0, course fields at the ctor's 6/0/4 —
and **class 1, tier 0**, unchanged at every poll from the first moment the event existed. The save state
taken from that running world (`./kit state ram 8`) holds exactly one resolved bike physics name:
`Bike\Race125P.phy` (plus `Bike\Wreckbike.phy`). This is the case the constant actually describes: a
freeride world with no `.cfg`, no career profile and no front end.

**3. The disc**, for the comment, not for the class: a tally over `MX.PSR` finds all sixteen
`bike\<tier><cc><A|P>.phy` names present, and mapping their offsets back onto the archive directory shows
they live in the bike model archives — `bike\race125a.phy` in 24 of them (`125sx`, `cr125*`, `kx125*`,
`rm125*`, `yz125*`, the `yz250f` four-strokes, `ghost_thq_01`), the 250 pair in 79, the 50 and 500 pairs in
5 and 4. `Tracks\free06_2.res` and `Tracks\free09_2.res` carry `bike\{race,pro}250{a,p}.phy` and
`bike\wreckbike.phy` only.

`crates/game-sim/tests/track.rs` keeps that in the suite: `every_physics_name_the_loader_can_build_is_on_the_disc`
walks every `Bike\*.res` on the disc (54 of them) and asserts all sixteen names `physics_file` can build are
there, asserts the freeride pair by name, and pins the freeride track's own bike physics to the 250 pair so
the misleading listing is recorded as a fact rather than as a conclusion. `./kit check`: 299 tests pass.

## Not verified / not 1:1

- **Which bike *model* archive supplies the pair is not chased, nor whether the request found a file at
  all.** The 125 pair is not in the freeride track's own archive, so it has to come from a mounted
  `Bike\<model>.res` — chosen by the bike selection / the `BikeNumber` key, which I did not follow. What is
  proved is the name the loader builds and asks for; the lookup that answers it is not.
- **Only the name is proved, not the contents.** No physics parameter was read out of either `.phy` file
  and compared against the running game, so "the bike behaves like a 125" is not shown here — only that
  the 125 Race pair is what gets asked for.
- **"Nothing on the freeride path writes the bike fields" is observed, not exhaustive.** I found no writer
  through the settings pointer chain other than the two above, but a front-end write through a cached
  event pointer would not match the grep I used, so I cannot rule out that the front end stores the same
  1/0 it found. In both worlds I measured the value a freeride world starts with is 1/0 either way.
- **A career profile is still unknown ground.** The constant is documented as the value absent one; I did
  not load a career save and the loader would honour whatever it put in those fields.
- **The tier is a `bool` in the port where the original keeps an int.** The loader branches `== 0` versus
  anything else, and the only writer that can set it writes 0 or 1, so a hypothetical third value would be
  Pro in both — not a difference I can reach.
- **Two of the five freeride areas.** `Free09` (the human's recording) and `Free06` (the scripted boot).
  `free07`, `free10` and `free11` were not booted; nothing in the loader is per-area, but that is read, not
  measured.
- **The loader's 0x104-byte buffers are not modelled.** `physics_file` returns a `String`; the original
  truncates at 260 bytes, which no name of this shape can reach.
- **The P-before-A order the loader builds the pair in is recorded in the comment only.** The port's
  function takes the tuning as an argument, so there is no order in it to match.
- **The case fold is assumed.** The loader asks for `Bike\Race125P.phy` and the archive holds
  `bike\race125a.phy`; the test folds case itself, as the rest of the file's lookups do, and I did not find
  the place in the original that does it.

## Next

Nothing outstanding for this task. For whoever wires the bike up: the pair is `physics_file(FREERIDE_ENGINE,
FREERIDE_PRO, …)`, and the open end above is which model archive it is read out of.
