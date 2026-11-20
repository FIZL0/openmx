# S492 — an AI bike knocks a prop while player 1 flies

2026-11-20. Task: `./kit plan S492`.

S479 moved the plane lane's props step after the riders from the call order alone. This run checks
it against an actual knock in the original.

## What I did

- New test `tests/ai/flown_knock.rs` `s492_an_ai_bike_knocks_a_prop_while_player_1_flies`.
  - biplane.p2m2's frame run whole in the interpreter (S328's recipe), neutral pad.
  - Ball 0 is put on the ground where AI rider 0x01881b10 rides at tick 9, and woken.
  - The bike's search is cleared once before tick 1, so it gathers the moved ball.
- What the test asserts at tick 10 (the knock):
  - The bike's collide leaves a kind-0 entry naming the ball.
  - The ball's struck listener wakes it right after.
  - The ball's list has the bike's mirror first.
  - The bike's resolves (in its own update) name the ball.
  - The ball's collection steps after every rider's update, the flying human's too.
  - The ball's two resolves run in that step, and the ball is knocked.
- Before tick 10, the bike's list never names the ball.

## Result

The port's order holds: `Sim::props_step_flown` after `Sim::step_flown_riders`. No port change.
Details under `# S492` in research/oracle.txt.

## Not verified / not 1:1

- The ball is woken by the test before the knock. An asleep ball struck by an AI bike is not run.
- The bike's gather is forced once. In a real ride the ball would already be in its list.
- The port's ball and bike are not compared bit for bit against this run. Only the order is.
