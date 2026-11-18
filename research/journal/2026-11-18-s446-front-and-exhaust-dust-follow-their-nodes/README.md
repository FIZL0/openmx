# S446 — front and exhaust dust follow their nodes

2026-11-18. Task: `./kit plan S446`.

The front sprays and the exhaust now take their points from their node chains (Shocks under Handlebars
under Bike; the exhaust pipe, which copies Bike's world), with the original's drive after the emitters.
Both are proved per tick in the Ee run of C33 when fed the original's clip pose. The Sim holds
Handlebars at the image's pose, so its front points differ in the last bits (S447). Details in
1-…-FINAL.
