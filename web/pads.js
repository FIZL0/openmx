// A pad the browser does not map "standard" (P244), read as one that is: the 17 buttons and 4 axes of the standard
// mapping, which index.html hands the worker. Such a pad's d-pad is often a hat on its axes and its triggers are axes
// too, so the standard indices alone lose them. Read here from the raw axes, by what they do rather than by the pad:
//   a hat switch    one axis that rests above 1 (chromium's 9/7) and reads -1..1 in eighths, up and clockwise
//   a hat pair      the last two of six or more axes, while they have only ever read -1, 0 or 1 (evdev's
//                   ABS_HAT0X/Y, Firefox on Linux), not a trigger
//   a trigger       an axis past the first two that read -1 the first time the pad was seen, rest at -1 and full at 1
// The d-pad goes to buttons 12-15 and the first two triggers to L2/R2 (6/7). With trigger axes and at most 11 buttons,
// the buttons are evdev's without L2/R2 or the d-pad (A B X Y L1 R1 Select Start Mode L3 R3, or without Mode), so
// those from 6 on move to their standard places. The sticks are the first four axes that are none of these. A
// standard pad is read as it is.
// A block: a classic script's top-level names would be the window's.
{
const seen = new Map();
// What a pad's axes were found to be, worked out once per pad from its first reading, the hat pair kept up as it reads.
const memo = g => {
  const key = `${g.index} ${g.id}`;
  let m = seen.get(key);
  if (!m) {
    const triggers = [];
    let hat = -1;
    g.axes.forEach((a, i) => {
      if (i >= 2 && a <= -0.9) triggers.push(i);
      else if (hat < 0 && a > 1.05 && a < 1.5) hat = i;
    });
    const n = g.axes.length;
    const pair = hat < 0 && n >= 6 && !triggers.includes(n - 2) && !triggers.includes(n - 1) ? [n - 2, n - 1] : null;
    m = { triggers, hat, pair };
    seen.set(key, m);
  }
  // A pair that reads between the steps is a stick after all.
  if (m.pair && m.pair.some(i => ![-1, 0, 1].includes(g.axes[i] ?? 0))) m.pair = null;
  return m;
};
// The pad as the standard mapping's 17 button values and 4 axes.
window.openmxPad = g => {
  const axis = i => g.axes[i] ?? 0;
  if (g.mapping === 'standard') return { buttons: Array.from({ length: 17 }, (_, i) => g.buttons[i]?.value ?? 0), axes: [0, 1, 2, 3].map(axis) };
  const m = memo(g);
  const raw = g.buttons.map(b => b?.value ?? 0);
  let buttons;
  if (m.triggers.length && raw.length <= 11) {
    const rest = raw.length === 11 ? [8, 9, 16, 10, 11] : [8, 9, 10, 11, 16];
    buttons = Array(17).fill(0);
    raw.forEach((v, i) => (buttons[i < 6 ? i : rest[i - 6]] = v));
  } else {
    buttons = Array.from({ length: 17 }, (_, i) => raw[i] ?? 0);
  }
  m.triggers.slice(0, 2).forEach((i, k) => (buttons[6 + k] = Math.max(buttons[6 + k], Math.min(1, Math.max(0, (axis(i) + 1) / 2)))));
  let up = 0, down = 0, left = 0, right = 0;
  if (m.hat >= 0 && Math.abs(axis(m.hat)) <= 1.01) {
    const d = Math.round((axis(m.hat) + 1) * 3.5) % 8;
    up = +(d === 7 || d <= 1);
    right = +(d >= 1 && d <= 3);
    down = +(d >= 3 && d <= 5);
    left = +(d >= 5 && d <= 7);
  } else if (m.pair) {
    left = +(axis(m.pair[0]) < 0);
    right = +(axis(m.pair[0]) > 0);
    up = +(axis(m.pair[1]) < 0);
    down = +(axis(m.pair[1]) > 0);
  }
  [up, down, left, right].forEach((v, k) => (buttons[12 + k] = Math.max(buttons[12 + k], v)));
  const special = new Set([...m.triggers, m.hat, ...(m.pair ?? [])]);
  const sticks = g.axes.map((_, i) => i).filter(i => !special.has(i)).slice(0, 4);
  return { buttons, axes: [0, 1, 2, 3].map(k => (sticks[k] === undefined ? 0 : axis(sticks[k]))) };
};
}
