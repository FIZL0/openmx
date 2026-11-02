// web/pads.js on pads the browser does not map "standard" (P244): each case a pad's raw buttons and axes over a few
// readings, and what the standard mapping's buttons and axes must read on the last. Prints a line per case and exits
// non-zero on the first that reads wrong. Run as `node pads.mjs <web dir>`.
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import vm from 'node:vm';

const window = {};
vm.runInNewContext(readFileSync(join(process.argv[2], 'pads.js'), 'utf8'), { window });
let index = 0;
const pad = (id, mapping, buttons) => ({ index: index++, id, mapping, buttons: buttons.map(v => ({ value: v })), axes: [] });
const read = (g, axes, buttons) => {
  g.axes = axes;
  if (buttons) g.buttons = buttons.map(v => ({ value: v }));
  return window.openmxPad(g);
};
const expect = (name, got, want) => {
  const bad = Object.entries(want).filter(([k, v]) => JSON.stringify(k.startsWith('a') ? got.axes[+k.slice(1)] : got.buttons[+k.slice(1)]) !== JSON.stringify(v));
  console.log(`${bad.length ? 'FAIL' : 'ok  '} ${name}  buttons ${got.buttons.join(' ')}  axes ${got.axes.join(' ')}`);
  if (bad.length) {
    console.log(`  wrong: ${bad.map(([k, v]) => `${k} wants ${v}`).join(', ')}`);
    process.exitCode = 1;
  }
};
const b11 = Array(11).fill(0);

// Firefox on Linux, an Xbox pad by evdev: LX LY LT RX RY RT HAT0X HAT0Y, A B X Y LB RB Back Start Mode L3 R3.
{
  const g = pad('045e-028e-Unknown pad', '', b11);
  read(g, [0, 0, -1, 0, 0, -1, 0, 0]);
  read(g, [0.2, -0.3, -1, 0.5, 0, -1, 0, 0]);
  const start = b11.slice();
  start[7] = 1;
  expect('evdev: rest, Start', read(g, [0.2, -0.3, -1, 0.5, 0.25, -1, 0, 0], start), { b6: 0, b7: 0, b9: 1, b12: 0, a0: 0.2, a1: -0.3, a2: 0.5, a3: 0.25 });
  expect('evdev: R2 full, L2 half, d-pad up-left', read(g, [0, 0, 0, 0, 0, 1, -1, -1], b11), { b6: 0.5, b7: 1, b12: 1, b13: 0, b14: 1, b15: 0, b9: 0 });
  const l3 = b11.slice();
  l3[9] = 1;
  expect('evdev: L3 is 10, not the d-pad', read(g, [0, 0, -1, 0, 0, -1, 0, 1], l3), { b10: 1, b12: 0, b13: 1 });
}
// Chromium on a generic HID pad: a hat switch on axis 9, resting at 9/7, the d-pad's eighths clockwise from up.
{
  const buttons = Array(12).fill(0);
  const g = pad('USB Gamepad (Vendor: 0079 Product: 0011)', '', buttons);
  const at = h => [0, 0, 0, 0, 0, 0, 0, 0, 0, h];
  expect('hat: rest', read(g, at(9 / 7)), { b12: 0, b13: 0, b14: 0, b15: 0, b6: 0, a0: 0, a3: 0 });
  expect('hat: up', read(g, at(-1)), { b12: 1, b13: 0, b14: 0, b15: 0 });
  expect('hat: right', read(g, at(-1 + 4 / 7)), { b12: 0, b15: 1 });
  expect('hat: down-right', read(g, at(-1 + 6 / 7)), { b13: 1, b15: 1, b12: 0 });
  expect('hat: down', read(g, at(-1 + 8 / 7)), { b13: 1, b14: 0, b15: 0 });
  expect('hat: left', read(g, at(-1 + 12 / 7)), { b14: 1, b12: 0, b13: 0 });
  expect('hat: up-left', read(g, at(1)), { b12: 1, b14: 1 });
}
// A cheap pad on Linux: LX LY RX RY HAT0X HAT0Y and 12 buttons of its own, no trigger axes.
{
  const buttons = Array(12).fill(0);
  const g = pad('0079-0006-Generic USB Joystick', '', buttons);
  read(g, [0, 0, 0, 0, 0, 0]);
  buttons[7] = 1;
  expect('pair: R2 button, right stick, d-pad down', read(g, [0, 0, 0.4, -0.6, 0, 1], buttons), { b7: 1, a2: 0.4, a3: -0.6, b13: 1, b12: 0 });
}
// Six axes that are two sticks and a third: the last two read between the steps, so they stay a stick.
{
  const g = pad('6 axes', '', Array(10).fill(0));
  read(g, [0, 0, 0, 0, 0, 0]);
  read(g, [0, 0, 0, 0, 0.3, 0]);
  expect('not a pair once it reads between', read(g, [0, 0, 0, 0, 1, 0]), { b15: 0, b12: 0, b13: 0, b14: 0 });
}
// A standard pad is read as it is.
{
  const buttons = Array(17).fill(0);
  buttons[12] = 1;
  const g = pad('Xbox 360 Controller (STANDARD GAMEPAD)', 'standard', buttons);
  expect('standard', read(g, [0.1, 0.2, 0.3, -1]), { b12: 1, b6: 0, a0: 0.1, a3: -1 });
}
