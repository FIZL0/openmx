# 1 — atv quad triangle scratch (FINAL)

Task K51. 2026-11-12.

## What I did

- K51 was filed as a load flake: `-p game-sim --test atv_quad` "failed, then passed alone" in S317's merge
  gate. It is not a load flake. Run alone in this slot on main 236563579 (load ~15), `sim_frame::` is red
  deterministically: `race_stoppie_short_replays_by_the_port` (frame 109, 27 words) and
  `race_wheelie_launch_replays_by_the_port` (frame 88, 21 words), with
  `the_sweep_that_hits_the_palm_is_the_ports` (frame 124, 27 words) the one the merge gate saw. Every
  word that differs is in 0x003dce50..0x003dcedc. This is the same failure P287 found and filed as MD143
  on its own branch (task/P287, pushed unticked, so MD143 never reached main's plan). I can't say why the
  runner's lone rerun passed; possibly it ran a different build. I did not look into it: the red is
  deterministic, so what matters is the cause.
- Fix: `sim_frame.rs`'s `ram_differs` leaves the range out (`TRIANGLE_SCRATCH`, 0xa0 bytes from
  0x003dce50), the same way it already leaves out the ground ring, with the reason below in its doc.
- Nothing in ps2kit or opentennis applied (this is a test-side compare of the original's scratch globals).

## What the original does

`./kit xref 3dce50 -p atv` (and every word to 0x3dceec): only two functions touch the range,
`./kit fn 0x00107050 -p atv` and `./kit fn 0x00109780 -p atv`, the mesh tree walk's two
box-against-triangle separating-axis tests (13 axes). 0x003dcef0 starts an unrelated object. Per call, each
writes before it reads:

- +0x40/+0x50/+0x60 (0x3dce90..): the triangle's corners, copied from the mesh (`./kit fn 0x00107790 -p atv`,
  floats or scaled shorts, w = 1), and read afterwards;
- +0x00..+0x30 (0x3dce50..0x3dce8f): the corners in the box's frame and their centre, stored (sqc2) before
  the axis loop reads them;
- +0x70/+0x80 (0x3dcec0, 0x3dced0): two edges, written on axis 0 before axes 1..12 read them (an axis-0
  reject returns before any read);
- +0x90 (0x3dcee0): the third edge's axis, written in axis 10's case just before it is read.

So the words carry nothing from one call to the next. The port's pair (atv_sweep / the part's Rust mesh
pair) keeps its triangle in its own frame and never writes them. The original leaves the last tested
triangle there, which explains both the zeros and (on the palm ride) the stale values the port shows.

## How it was proved

Static: the xref of every word in the range and the two functions read in full for write-before-read
(above). A whole-ELF scan backs the xref: `./kit dis 0x100000 0x540000 --program atv` (1.1M lines). Every
instruction with an immediate in -12720..-12561 whose base register was loaded with `lui 62` within the
40 instructions before it is in those two functions (82 hits, 41 each). No `$gp`-relative access reaches it:
$gp = 0x003dbaf0 (entry, `lui 62; addiu -17680`), so the range would be $gp+4960..5119, and no instruction
uses those offsets. Test: `./kit check --package=game-sim --test=atv_quad sim_frame::` green alone after the change,
then the gate.

## Not verified / not 1:1

- The port does not write the original's triangle-test scratch at 0x003dce50..0x003dceef, and the compare no
  longer checks it. Write-before-read is proved statically, from the decompilation and a whole-ELF scan, not
  by a watch in a running original. Code that reached the range through a pointer computed some other way
  than `lui`+offset or `$gp` would escape the scan. I know of none.
- Why the runner's lone rerun counted the binary as passing: not looked into (see above).

## Next

- MD143 (on task/P287's plan only) is this same work. If P287 merges, MD143 can be closed against K51.
