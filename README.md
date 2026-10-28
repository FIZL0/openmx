# openmx

A re-implementation of **MX Unleashed** (PS2, SLUS-20767) in Rust — the original's simulation, ported
function by function and **proved to match it bit for bit**, behind a new renderer, new menus, local
split-screen and online play, on the desktop and in a browser.

It ships no game data. Everything it shows — tracks, terrain, bikes, the rider, textures, menu art,
sound — is read at runtime from your own dump of the disc.

```
cargo run --release -p openmx                          # the front end: pick a track and ride it
cargo run --release -p openmx -- --track free06        # straight onto a track, no menu
cargo run --release -p openmx -- --players 2 --split v # two players, side by side
cargo run --release -p openmx -- --vehicle biplane     # start in the track's biplane
```

## What is in it

**The ride.** The bike's ground step — suspension, tires, the rigid body, engine torque, slip, the air
state and the collision sweep — on the parameters each bike's own `.phy` carries. The rider on top of
it: the trick machine and its 24 stunts from the disc's `stunts.ini`, landings judged against the
`.phy`'s limits, wrecks, the ragdoll and the put-back. The track's biplane, with its own flight model,
contact search and crashes. The follow and wreck cameras off each track's `cpresets.ini`. All of it
runs through `ps2-float`, so a ported calculation gives the original's number, not a nearly-right one.

**The shell.** A GPU renderer (wgpu: Vulkan, Metal, DX12, GL, WebGPU) drawing the scene's props, eco
foliage, terrain, bike and skinned rider at any resolution and refresh rate, interpolating between the
simulation's 30 Hz ticks; a software rasteriser as reference and fallback. The original's HUD,
speedometer and radar off its own art, plus a debug overlay (**H**). Music and engine sound. A front
end in the original's style — press-start splash, Free Ride, Garage, Settings, pause — with settings
kept in `settings.txt` and `controls.txt` beside the disc.

**Players.** Up to eight on one screen, each with their own pad, controls preset, bike and rider. Online
play by rollback (ggrs) over iroh QUIC: room codes, LAN discovery and quickplay, up to sixteen seats,
with a handshake that refuses a peer whose build would ride differently.

**The browser.** `tools/web.sh` builds the same shell to WebAssembly in `web/`; the page keeps its
copy of your disc in the browser's private storage and plays online too.

**iPhone and iPad.** The iPhone build is on its way through FIZL0's app source: add
`https://fizl0.github.io/apps/source.json` under Sources in Feather, AltStore or SideStore
(or tap one on [openmx.dev](https://openmx.dev/#get)) and openMX appears there when it lands.

**Additions, layered on top.** Track and music mods (`mods/`, `--mods`), including tracks imported from
MX vs. ATV and Untamed, gameplay modifiers, markers you can drop and return to, camera and FOV
settings. With them off the ride is the original's, and the replay tests say so.

## Controls

The default layout is new: **R2** gas, **L2** brake (both analogue), left stick to steer and lean,
right stick to look around, **Select** for the camera. `--controls original` (or the Controls screen)
puts back the original's: **Cross** throttle, **Square** brake, L2 clutch, R2 front brake.

Tricks are the original's: in the air, hold one, two or three of **Circle**, **Triangle** and **R1**
and push the **d-pad** — that names one of the 24 stunts. **L1 + R1** puts the bike back. **M**
switches between the bike and the biplane. **Start** pauses.

A keyboard works too — X/Space, Z, the arrow keys, C/V/D for the trick buttons, A+D for the reset
(also WASD/HJKL layouts). `--throttle r2` and `--pad-pressure` arrange analogue gas on pads that do not
report button pressure.

## Running it

You need your own dump of the disc. Copy `kit.toml.example` to `kit.toml` (or run `./kit setup`), set
`[game] serial` to `SLUS-20767` and `iso` to your image's path; the shell checks the disc's serial before it reads anything. Then `cargo run --release -p openmx`.
`--help` lists the rest: `--size`, `--scale`, `--render-scale`, `--renderer gpu|software`, `--vsync`,
`--fps-limit`, `--online`/`--peer` for a direct online game, and more.

`--record FILE` writes every frame's pad down as a tape and `--play FILE` rides it back; the result is a
function of the tape, so a bug report becomes a file anyone can ride. `--play` also takes a `.p2m2`
recorded on the original in PCSX2. Tapes hold game-derived values, so keep them under `context/`.

## How it is built

| Crate | What it is |
|---|---|
| `crates/ps2-float` | The PS2's arithmetic: the EE FPU, VU0 and the SDK's libm, bit-exact, `no_std` |
| `crates/ps2-oracle` | A small R5900 interpreter that runs the original's own functions off your disc, for tests |
| `crates/game-sim` | **The port**: the disc's formats, the ride, the rider, the biplane, the camera, replay. No IO, no host floats |
| `crates/game-render` | Disc → meshes and textures → pixels, on the GPU or in software |
| `crates/openmx` | The shell: window, pads, audio, menus, split screen, online play, the browser build |
| `crates/openmx-net` | Finding and joining a game: iroh QUIC, mDNS, room codes, quickplay |

The simulation is held to the original by four kinds of proof: **function oracles** (the port and the
original's own code side by side in `ps2-oracle` over 10,000+ generated inputs), **live captures** of the
original running in PCSX2, **save-state RAM**, and the **replay harness** (same input, same state hash,
frame by frame). Rendering, menus and presentation are matched by eye, not 1:1. `AGENT.md` sets out
where the line is.

The reverse-engineering workbench it is built with — `./kit`, from
ps2-decomp-kit — is in the repo too: disc and ELF readers,
headless Ghidra with one-function-at-a-time reading, a PINE client and capture recorder for the running
game, and the task list and journal the work is done through:

```
./kit check                     # the tests, summarised to one line
./kit plan next                 # the next task in plan/TODO.md
./kit fn <addr>                 # one decompiled function
./kit pcsx2 start               # the original, with live memory access
./kit record <spec> --frames N  # capture it
./kit done <task> "<summary>"   # test, audit, tick, commit
```

Every finished task leaves an entry in `research/journal/`, each ending with what could not be verified
against the original.

## Documentation

| | |
|---|---|
| [AGENT.md](AGENT.md) | the rules the work follows: what must be exact, what may not be committed |
| [SETUP.md](SETUP.md) | setting up the workbench: disc, BIOS, PCSX2, Ghidra |
| [PLAN.md](PLAN.md) | how a task goes from the list to a commit |
| [docs/WORKFLOW.md](docs/WORKFLOW.md) | the port loop: find the code, port it, prove it |
| [docs/PROVING.md](docs/PROVING.md) | the four proof methods, and which to use when |
| [docs/PS2.md](docs/PS2.md) | the hardware, and why it bites |
| [docs/GHIDRA.md](docs/GHIDRA.md) | decompiling PS2 code well |
| [docs/PCSX2.md](docs/PCSX2.md) | driving the emulator, save states, recordings |
| [docs/CONTEXT.md](docs/CONTEXT.md) | keeping a long agent-run project affordable |
| [docs/TROUBLESHOOTING.md](docs/TROUBLESHOOTING.md) | the failures that actually happen |
| [docs/TEXTURES.md](docs/TEXTURES.md) | upscaled and edited textures: packs, replacements, chaiNNer |
| [crates/README.md](crates/README.md) | the Rust side's rules: floats, the oracle, dependencies |
| [plan/HUMAN.md](plan/HUMAN.md) | the things only a person can do (dump the disc, make the states) |

## Game data

None of it is here and none of it may be. Dump your own disc and BIOS. Everything game-derived —
extracted files, decompilation, captures, tapes, screenshots — lands in `context/`, which is
git-ignored, and `./kit audit` keeps retail addresses out of the shipped code.

## Licence

MIT — see [LICENSE](LICENSE). It covers this code, not MX Unleashed, which belongs to its owners.
