# MD80 — MX vs. ATV's biplane and Kiowa

2026-11-03. Task: `./kit plan MD80`.

MX vs. ATV Unleashed's biplane and Kiowa against MX Unleashed's. Each flier has two models: model one is MX
Unleashed's machine, model two is a new one for the 3/4 skins. The .phy files keep MX Unleashed's values and
read the model-dependent keys as ModelOne*/ModelTwo*. SLUS-21104's flight code is MX Unleashed's, with fused
multiply-adds throughout; the Kiowa's aero step was rewritten, and the wreck checks moved. Written up in
research/mxvatv-pc.md "The biplane and the Kiowa (MD80)" and research/oracle.txt `# MD80`. Nothing is
captured or ported yet. That is split: MD80a captures, MD80b the biplane's step, MD80c the Kiowa's, MD80d the
mod pack, MD80e in the game.
