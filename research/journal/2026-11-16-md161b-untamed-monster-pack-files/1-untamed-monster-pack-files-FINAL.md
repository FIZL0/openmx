# 1 — untamed monster pack files

Task MD161b. 2026-11-16.

## What I did

- openmx-mods, branch `openmx-MD161b` (commit 249c8f1, on top of `openmx-MD144`, which carries MD163, MD81e
  and MD81f's `untamed_cars` work; none of it is on openmx-mods main yet):
  - `tools/import-atv/src/untamed_cars.rs`: `CARS` gains `("monstertruck", 5)`, first, in kind order. The
    pack now copies SLUS-21701's `res\veh\VehicleType_5.res` whole beside types 6–8 (91 files), and
    `mod.json` lists the truck as `{"car": "monstertruck", "phy": "Veh/monstertruck.phy.bpb", "psg":
    "Veh/Anim/monstertruck.psg", "col": "Veh/Col/MonsterTruckM.col", "step": "untamed"}`.
  - `tests/disc.rs` (ignored, needs the Untamed iso): names, each `.phy.bpb`, modcheck clean, and
    `monstertruck: 55 nodes, tires read, Weight 3500`.
- openmx: `crates/openmx/tests/untamed_monster.rs`, a copy of MD81f's sandrail shell test for the truck.
  It rides `--mxvatv-car monstertruck@untamed_cars` headless on free09 and checks:
  - the pick line says "on MX vs. ATV Untamed's step";
  - the truck goes over 100 ft on the gas (it went 446.6 ft);
  - the pack's `.phy.bpb` has `[Preload]` off (MaxPreload and MaxPreloadTime 0, Untamed's retune);
  - a `Load::untamed` Ride made here off the same pack files, rigged on `MonsterTruckM.col`, stepped
    on the same pad, meets the shell's body words exactly one tick per switched-in tick.
  - It skips with no disc or no `mods/machines/untamed_cars` (off by default: the pack is opt-in).
- Took nothing from opentennis: it has no machine packs.

## What the original does

Nothing new: the truck's archive and `.phy.bpb` are the disc's, copied whole. MD161 ported the step.

## How it was proved

- `import-atv` disc test against the MX vs. ATV Untamed iso: passes, modcheck clean.
- `untamed_monster.rs` passes with the pack built into this slot's `mods/machines/` (git-ignored).
- Unlike the sandrail test, this one rigs the Ride on the pack's `<car>M.col`: without it the truck's
  rays (MD149b) made the shell's ride differ from the local one, so the shell does use the record.

## Not verified / not 1:1

- **No drive of the original's monster truck is replayed.** That needs MD161a's Untamed car frame
  harness (MD160's drives); this task only proves the shell rides the pack bit for bit as the port's
  `Load::untamed` Ride, not that the port matches SLUS-21701.
- **Spawn image not compared.** MD163/MD81f matched their car built off the pack against a spawn image
  of the original; no such image of the monster truck was compared here (MD160 has images; MD161a).
- **The gate does not ride it.** The shell test skips without `mods/machines/untamed_cars`, which only
  exists where someone ran `import-atv untamed-cars`; the openmx-mods disc test is `#[ignore]`d.
- **Not looked at beside the original in PCSX2** (pack todo T7); nothing of the truck is drawn or heard.
- **openmx-mods `openmx-MD161b` is unmerged**, stacked on the unmerged MD39c → MD163 → MD81e → MD81f →
  MD144 chain.

## Next

MD161a (the harness and MD160's replay).
