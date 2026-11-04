# F31b4c — mass

2026-11-04. Task: `./kit plan F31b4c`.

MD63e's `[Mass Properties]` where SLUS-21104's bike differs from MX Unleashed, ported as
`handling::mass` (the part reader and the bike loader's re-read). Every word the reader writes matches the
original on 10,000 generated files; state 4's 134 mass words come out word for word from
`mx_race250p.phy` (with the model box fitted from the record's inertia); ride-2 and flip-1 stay bit-exact
on the port's words. The product (`mxvatv_phy`) is not moved onto it yet; that is MD63e's. Details in
1-mass-FINAL.md.
