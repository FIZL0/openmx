# V138: release 0.19.0

Taken in a control session (`./kit take V138`): the run kept starting other tasks ahead of the queued release.

- Wrote RELEASE_NOTES.md's player bullets from the commits since v0.18.0 (photo mode, auto photos, first-person
  camera, camera sliders, dual-stick leaning, physics editor, parked machines struck and pushed, AI riders round
  split screen's bikes, race outro and crowd, quad exhaust, frame pacing).
- `OPENMX_RELEASE_BRANCH=task/V138 tools/release.sh 0.19.0 publish`: v0.19.0 published with 11 assets,
  https://github.com/FIZL0/openmx/releases/tag/v0.19.0; FIZL0/apps rebuilt.

Failed first: the Windows build stopped with "windows build needs (once): sudo pacman -S mingw-w64-gcc; rustup
target add x86_64-pc-windows-gnu". MinGW is installed; the shell's PATH found Arch's /usr/bin/rustc, which has no
Windows std. rustup's toolchain in ~/.cargo/bin has it: run the release with `PATH=$HOME/.cargo/bin:$PATH`.

**Not verified**: the downloads were not started on Windows, Mac, iPhone or Android (no such devices here); the
release run of pages.yml (openmx.dev's site) was still going when this was written.
