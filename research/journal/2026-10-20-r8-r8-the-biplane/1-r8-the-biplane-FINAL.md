# 1 — r8 the biplane (FINAL)

Task R8. 2026-10-20. Research only: I added names to `research/oracle.txt` (the "R8 — the biplane" block at
the end). No code changed. The emulator was never started.

## What I did

- Read the biplane's class, constructor, vtables, loader and per-tick slots with `./kit fn` / `./kit dis`.
  Several MXBiplane slots are not analysed by Ghidra, so I read those as disassembly.
- Dumped the shipped `Biplane\biplane.phy`, `biplaneparticles.dat`, `machine.txt` and
  `appstart\biplanecpresets.ini` out of `MX.PSR`. I used a scratch reader that applies the archive layout from
  the D5 journal (`.res` = packed u16 block sizes, a directory, then zlib blocks). The copies of `biplane.phy`
  in `machine.res` and `Tracks\free06_2.res` are byte-identical.
- Read the shipped command bindings out of the `acc-start` save state's RAM. They give the input manager
  `*(0x0041f770)+0x104`, map `+0x648`, first bindings at `+0x138 + pad*0x138 + cmd*4`.
- Followed the freeride switch:
  - **Trigger:** a proximity scan (`./kit fn 3a8330`).
  - **Prompt:** `./kit fn 3a7210`, `./kit fn 360d30`.
  - **Answer:** `./kit fn 3601b0`.
  - **Swap:** `./kit fn 210b90`.

## What the original does

**The class.** The Biplane base is `./kit fn 3505f0`. MXBiplane (`./kit fn 353d10`, 0xea0 bytes) sits on top
of it, and the vehicle factory `./kit fn 20eb10` builds it as machine kind 1.

The per-tick work is spread over vtable slots +0x1a8, +0x1ac and +0x1b0:
- **+0x1a8, pre-force:** the control step `./kit dis 352160 352460`.
- **+0x1ac, forces:** the autopilot `./kit dis 352460 3527f0`, the engine and the aero.
- **+0x1b0, measure:** airspeed, alpha/beta and Euler angles. It also runs the surface animation and the wreck
  check.

The three slots are run by the shared rigid-body step. The oracle block lists every slot.

**The loader.** `./kit fn 350a70` builds the following, opens `Biplane\biplane.phy` and calls the reader
`./kit fn 3511f0`:
- an engine;
- an aero model;
- three tires, two in front with a brake and one at the rear with a steer;
- two prop-cone models.

The oracle block lists every key and where it lands. MXBiplane's reader adds `[Animation] PelvisBoxPosition` and
`PelvisBoxSize`.

Shipped values:
- Weight 1500, MaxPower 250.
- MaxElevator 30, MaxAileron 35, MaxRudder 20.
- ThrottleIncrement 0.35, ReverseThrottle -0.2.
- Control rate limits: aileron 9, elevator 9, rudder 1.
- Wrecks: WreckTime 5, MinWreckSpeed 50, WreckDamping .999, WreckInertia .5, WreckGravity 1.5.

**The pad.** The input slot `./kit fn 351e20` takes the left stick x/y in f12/f13. It turns eight commands into
five fields:

| Command | Shipped button | Field |
| --- | --- | --- |
| 0x0e | Cross | throttle-up (analogue) |
| 0x0f | Square | back |
| Up / Down | d-pad | elevator ±1 |
| Left / Right | d-pad | aileron ±1 |
| 0x14 / 0x15 | L1 / R1 | rudder ∓1 |

- The stick values are cubed and negated.
- The d-pad overrides the stick.
- The help screen's eight labels name them: Forward, Back, Climb, Descend, Bank Left/Right, Left/Right Rudder.
- L1+R1 together (commands 0x4c/0x4d) is the reset chord. It only works when the plane is on the ground or
  wrecked.

**Crashes.** `./kit fn 353180` checks for two kinds of crash.

*Reason 1, a crash on contact:*
- The trigger is body contact faster than MinWreckSpeed, or any contact while upside down.
- Five seconds (WreckTime) later the plane is unwrecked and reset.

*Reason 13, the map edge:*
- The trigger is coming within 150 of the terrain's edge.
- The plane is thrown back toward the centre with a random spin and half gravity scale.
- It waits 2.5 s on the ground, then WreckTime, then resets.

During a wreck (`./kit fn 3535d0`):
- the body takes WreckInertia, WreckDamping and WreckGravity;
- the throttle and the engine die and the engine sound stops;
- the prop shows `propconebent`.

For a player, MXBiplane adds a pad rumble. The reset puts the plane back at its spawn transform.

**Animations.**
- The rider build registers `biplane_roll_bck` / `biplane_roll_fwd` for every biplane and helicopter
  (`./kit fn 248150`).
- The rider pose blends the two clips (`./kit fn 2492b0`).
- MXBiplane finds `Dummy_Foot_L/R_pos` and `Dummy_Hand_L/R_pos` for the pilot's hands and feet.
- The surfaces are bones: four wing flaps, two rear flaps, the rudder and `PropDummy`.

**Model, textures and sounds.**
- Models are `biplane`, `biplane2`, `biplane_ss` and `biplane2_ss` (`.psg` and `.col`), plus the two prop cones.
- Textures are `pitts_*`, `guns`, `biplaneprop`, `biplanepropbroken` and `specular04`.
- Sounds are `carengine.aud` "biplane" with 7 loops, and machine.aud `biplaneTireParams`,
  `biplaneTireTouchdown` and `biplaneWreck`.

**Emitters.** `biplaneparticles.dat` in each freeride track's archive defines:
- wreck dust and chunks;
- smoke from the left exhaust;
- two exhaust emitters;
- three tire-dust emitters;
- prop-wash dust (type HelicopterDust, on `PropDummy`).

**Camera.** Freeride (mode 5) only. `./kit fn 20a1c0` builds one camera-preset object per machine kind, and kind
1 reads `AppStart\BiplaneCPresets.ini`. It holds five presets: "Flight Cam 1", "Flight Cam 2", "Slot Cam",
"Lookback Cam" and "1P Stunt". Each has split-screen and widescreen variants.

**Spawn.** Machine placement `./kit fn 20f6e0` reads `<track>\machine.txt`. When the event's vehicle kind for a
player is 1, it reads `[Biplane<n>]`. For free06 that is `2304, 135, 2281` facing `0, 0, -1`. The same file
places a career biplane as `[Machine4]`. free06, 07, 09, 10 and 11 all have Biplane sections.

**The switch.**
1. Every frame, `./kit fn 3a8330` looks for the nearest machine of each unlocked kind to the rider.
2. A driverless one within 30 ft becomes the HUD's candidate.
3. `./kit fn 3a7210` asks "Would you like to switch to the Biplane?" (`STR_SWITCH_BIPLANE`). It does not ask if
   the rider is wrecked or faster than 40 mph.
4. The answer `./kit fn 3601b0` calls the swap `./kit fn 210b90`. The pause menu's "Switch To Machine: %s" calls
   the same swap.

What the swap does:
- **The bike:** it loses the player's controller. Its model nodes are hidden. Nothing moves it, so it stays
  where it was left.
- **The plane:** it gains the controller and its own camera, and is unwrecked and reset.
- **Spawn:** the plane spawns where `machine.txt` put it, not next to the rider.

## How it was proved

Static reading only: decompilation and disassembly. Shipped data comes from the disc's archives, and the shipped
bindings from one save state's RAM (`acc-start`). No function oracle and no live run. This is a research task
(t: none). The block is a map for S33–S36, I6, V21 and P19 to prove against.

## Not verified / not 1:1

- **Nothing ran live.** Every claim is a static reading, so field roles are inferred from the code around them.
- **The stick axes.** That f12/f13 are the left stick's x/y is inferred from where they come from: the
  controller's first two axis objects (+0x88/+0x8c), next to camera_nudge's documented right stick in f14/f15.
  The sign of "Climb" against stick up is not checked.
- **Which label goes with which command.** The help screen's labels were matched to commands by meaning only.
  The screen's layout was not read.
- **The bindings.** They were read from one save state on pad 0 and pad 1. A profile with remapped controls
  would differ.
- **The button that answers the prompt.** `./kit fn 3601b0` is reached only through a vtable, so I did not
  trace which pad command calls it. Its `+0x44` "no" arm (`./kit fn 21c1b0`) was not read either.
- **The bike while switched out.** The bike is hidden and loses its controller, but I did not check whether it
  keeps simulating or is frozen. `0x0027b9f0` and `0x00200f00` were not read beyond their first lines.
- **Unread MXBiplane slots.** `0x00355f40` (input override), `0x003560c0` (+0x40), `0x00355290` (measure
  override) and `0x00354e00` (called by reset) were not read, so their effect on input and measurement is not
  mapped.
- **What feeds the pilot-pose blend.** I did not trace which biplane fields feed the rider's `+0x488`
  `+0x170/+0x174` (the roll_bck/roll_fwd weights).
- **`biplaneprop02`.** The brief mentions it, but no archive member has that name. The prop textures that do
  exist are `biplaneprop` and `biplanepropbroken`.
- **The `_ss` models.** I took them to belong with the `free*_ss` tracks. That they are a split-screen or
  low-detail variant is unverified.
- **`MinAltitude`.** It is in `biplane.phy`, but the biplane reader never reads it. Another class may own the
  string.
- **The map-edge test.** I took reason 13's `0x001a6e50(150, terrain, pos)` to be "near the terrain edge" from
  its use. I did not read its body.
- **The camera presets.** Only the five single-player 4:3 presets were read in full. The h/v/w/x variants were
  seen but not transcribed.

## Next

R8 is research only. The follow-up tasks (S33–S36, I6, V21, P19) should start from the oracle block and prove
each piece against the original. Three gaps can be closed with a short live session:
- the prompt's answer button;
- the stick sign;
- the bike's state while switched out.
