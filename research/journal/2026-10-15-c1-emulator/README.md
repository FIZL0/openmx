# C1 — emulator

2026-10-15. Task: `./kit plan C1`.

The game boots and plays under the kit, on a kit-managed copy, driven entirely by `./kit` commands:
`./kit pcsx2 copy N` → `start` → `./kit pad send` → `./kit shot`, with `context/shots/c1-title.png` the
main menu as proof and two copies running side by side at different points in the boot. Getting there
turned up three kit defects — a fresh PCSX2 config boots the setup wizard instead of the ISO; a kit copy's
controller port 1 stays bound to a keyboard the kit then disables, so the game gets no input at all; and
the window-grab screenshot path can hang forever and take the working fallback with it. All three are
fixed in `ps2kit/` and filed upstream. The game's own controls diagram was captured on the way
(`context/shots/c1-controls.png`) and is transcribed in `1-emulator-FINAL.md` — S3 needs it.
