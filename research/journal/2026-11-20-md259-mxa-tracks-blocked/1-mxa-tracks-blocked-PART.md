# 1 — mxa tracks blocked (PART)

Task MD259. 2026-11-20.

## What I did

- The task was not in this worktree's plan: it came back to origin/main in fe1bee7b3 (restored old MD70)
  after the slot was cut, so I fast-forwarded task/MD259 to origin/main.
- Looked for the reference the task names: `context/mxa` (NOTES.md, MXA.TrackCore TrackDocument /
  TrackPackage / TrackFiles) and any `.mxatrack` file. Not on this machine: not in the shared context, the old
  slot contexts (`openmx-slots/kept-context`, `old-context-s1..s3`), `~/runs`, `~/dev`, `~/Work`,
  `~/Documents`, `~/Downloads` (find under `/home/rycs`, depth 7), nor in the user's GitHub repos
  (`gh repo list`). No commit ever mentions `mxatrack`.
- Without the format or a sample file, a reader would be a guess, so no code. Blocked for the person, with a
  wizard at `context/notes/wizards/MD259.sh`.

## What the original does

Not applicable (another program's file format; nothing to read).

## How it was proved

Nothing to prove; no product change.

## Not verified / not 1:1

- The whole task: no `.mxatrack` reader, no track mod, no test. Reason: MXA's source and any sample
  `.mxatrack` are missing from this machine.

## Next

The human: copy MXA's source to `context/mxa` (with NOTES.md if there is one) and a Track Studio
`.mxatrack` to `context/mxa/samples/` (`./kit wizard MD259`). Then the next session writes the reader into
`crates/openmx/src/track_mods.rs` from TrackDocument / TrackPackage / TrackFiles and a test that loads the
sample as a track mod and rides it.
