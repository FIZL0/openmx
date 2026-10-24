# 1 — trick repeat count (FINAL)

Task P140. 2026-10-23.

## What I did

- `crates/openmx/src/ride_hud.rs`: `folded()` turns the chain's names into its entries. A trick done
  again straight after itself becomes one entry with its count, `Backflip x2` (one space, a lower-case
  `x`, the same style as the multiplier's `x2` beside the points). `Tracker::popup` gives the folded
  entries to `Popup::names` while the chain is going and while a bailed chain plays out. `trick()`'s
  latest-three window and its leading `…` now count folded entries, so three backflips and a can-can
  read `Backflip x3 + Can Can`.
- Only **back-to-back** repeats fold. Backflip, can-can, backflip reads `Backflip + Can Can + Backflip`.
  Folding every repeat of a name would lose the order the chain was ridden in, and the original gives
  no rule to follow (see below), so this is my choice.
- Names fold when their **displayed text** matches, not their ids. Two different stunt ids that show
  the same name read as a repeat, and a variation with a different name does not fold. The rider sees
  the text, not the ids.
- The count grows in like a new name. A repeat moves `popped`, as any new node does, so the folded
  entry, which is the newest, pops in from half size. The original also rolls every new node in as a
  new line: its text setter swaps between two text slots by the node count's parity, repeats included.
- The score is unchanged. score.rs keeps one node per trick, and the multiplier (`Popup::mult`, the
  combo's `xN` beside the points) still counts every trick: `Backflip x2` over `… x2`.
- hud.rs (the debug overlay) shows only the last trick's name, and Original's drawn HUD (overlay.rs)
  draws no trick chain. Neither needed a change.
- opentennis has no trick chain, so nothing was taken from it.
- Test: `crates/openmx/tests/ride_hud.rs` `a_repeated_trick_reads_with_its_count`. It checks the
  backflip, backflip case (`Backflip x2`), backflip ×3 + can-can (`Backflip x3 + Can Can`), the broken
  run, an unchanged non-repeating chain, the window with `…` over folded entries, a single trick, the
  multiplier still counting every node, the pop-in, and a bailed chain playing out still folded. It
  checks both `Popup::names` and the text the HUD draws.

## What the original does

Its trick text setter (`./kit fn 0x002a5660`, called from the line update `0x002a54f0`; P121's block
in research/oracle.txt) never builds a chain of names. Its two text slots, `STR_STUNT%d_NAME_%d` and
`STR_STUNT%d_SCORE_%d`, alternate by the parity of the combo's node count (card +0xcc). The new node
goes in one slot and the one before it plays out of the other. The formats (`./kit data 0x436b20 0xb0
--as hex`):

- name: `%s`, the stunt display name of the newest node's id. It is `BackFlip + %s` when the overlay's
  +0xa0 flag is set, which means a trick ridden inside a backflip, for example `BackFlip + Taunt`. That
  is a different `+` from ours: two moves in one node, not a chain.
- points: `%d` for one node, and from three nodes `%d X %d` (the older node) and `%d  X %d` (the
  newest). With two nodes the film shows `10000 X 2`, so the `X n` is the combo's **length**.
- a landed chain of two or more: `%d Stunt Combo Total` over the banked total.

So what the user remembers as "backflip x2" is the original's newest name, `BackFlip`, over its points
and `X 2`, the combo's length. That `X 2` counts every trick, repeat or not. The original:

- never folds repeats and never spells out a chain;
- on a broken run (backflip, can-can, backflip) shows `BackFlip` over the backflip's points `X 3`, with
  the can-can before it rolling out of the other slot. Repeats and broken runs look the same to it;
- on a variation shows that variation's own display name.

The folded chain is therefore our HUD's own presentation (P121 made the HUD ours, from
docs/hud-concept.html). It keeps the user's `Backflip x2` and keeps the multiplier counting the combo,
as the original's `X n` does.

Film: P121's `recording2.mp4` (s5 `context/shots/P121/`, git-ignored; it is the human's recording2
played under Video Capture), at 5.6–6.2 s, 10 fps, cropped. It shows `BackFlip + Taunt` / 3240…3990,
then `Taunt` / `2080`, then `BackFlip` / `10000 X 2`, then `2 Stunt Combo Total` / `24160`. At 8–10 s
it shows a single `CanCan` / 480…3740 with no `X`.

## How it was proved

- The original's rule: read from the decompiled text setter and its format strings (above), and checked
  against frames of P121's Video Capture film of the human's recording2 (the frames above). That film
  has no back-to-back repeat. See below.
- Ours: `./kit check -- -p openmx --test ride_hud` passed (30 tests, including the new one), and then
  the gate in `./kit done`.

## Not verified / not 1:1

- The folded text (`Backflip x2`, `Backflip x3 + Can Can`) is not 1:1 and cannot be. The original draws
  no chain, only the newest name over `points X n`. Folding back-to-back repeats only (not every repeat
  of a name) and folding by displayed name are my choices, made for the reasons above.
- I did not film a fresh PCSX2 run of backflip ×2, ×3, and backflip / can-can / backflip as the brief
  asked. The decompiled setter shows that none of these changes the name text: it prints the newest
  node's name whatever came before. The existing film confirms the setter's formats on a two-trick
  combo. A ride of the exact sequences would need a generated .p2m2 that lands repeated backflips, and
  for a text the code shows cannot fold, that cost was not justified.
- The original's `X n` on the newest node from three nodes on uses a format with two spaces
  (`%d  X %d`), and the decompile lists only one argument for it. What it prints at three or more was
  not filmed.
- Our count's pop-in is the newest name's existing pop (P121's timings, by eye). The original rolls the
  repeat in through its two-slot Stunt movie; the motion was not compared frame by frame.

## Next

Nothing.
