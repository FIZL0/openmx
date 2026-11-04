# P293 — install status popup

2026-11-04. Task: `./kit plan P293`.

Ported opentennis's store transfer status (M136, M141, B601, transfers.rs) into openmx's menu. Store
downloads and uploads under way now show as a pill at the top middle of every menu screen. When a
batch ends having changed the mods, a Reload / Later popup comes up. If it is put off, a Reload Mods
button (Select) waits there and Play asks once more. Checked with unit and menu tests and two
llvmpipe screenshots, but not with a live store download. Details are in 1-install-status-popup-FINAL.md.
