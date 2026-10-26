# V79 — biplane surfaces smooth

2026-10-26. Task: `./kit plan V79`.

The user saw the biplane's flaps, elevator and rudder move, but not as smoothly as in the original.
The original's ease turned out to be the control law's rate limits, which the port already matches. The
aileron matches bit for bit on every row of the human's flight. The original also draws once per 30 Hz
tick (Video Capture: two identical frames a tick). The port's pictures run faster than the ticks and
already slid the plane between ticks, but they posed the surfaces from the last tick alone. Now the
shell blends the surfaces between the last two ticks as well (`game_render::biplane::between`,
`plane_draw::shown`), so they glide along the original's ramp. Tests are in game-sim, game-render and
openmx.
