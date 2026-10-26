# S110 — riding wreck entry

2026-10-26. Task: `./kit plan S110`.

The riding ragdoll (S109's `ragdoll_riding::Riding`) moved from the shell into the rider
(`Rider::riding`, checksummed and on the wire), stepped each tick while the shell hands the rider its
grips (`Rider::holds`, offline only), and the wreck now enters from its bodies (`Riding::hand_over`
into `Down::enter_from`) instead of the animated skeleton. Against run2's wreck (verdict at 3716,
entry at 3718) the port's entry from the riding bodies is the original's within S70's NEAR on all
216 lanes, the throw and the generator equal; the riding ticks before it are the original's bit for
bit. Drawing the riding ragdoll stays behind `OPENMX_RIDING_RAGDOLL=1`.
