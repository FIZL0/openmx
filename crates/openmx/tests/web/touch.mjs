// The on-screen pad (P197; crates/openmx/tests/web.rs, `the_touch_pad_rides_on_a_phone`) in headless chromium with a
// phone's touch screen emulated: web/ served on loopback at a phone's size, the page told the game started (no disc
// and no game module: the pad is the page's own), then fingers put down through CDP's touch input and the pad read
// back from `navigator.getGamepads()`, as index.html reads it for the worker.
//
//   node touch.mjs <web dir> <out dir> <scratch profile dir>
//
// Writes landscape.png and portrait.png into <out dir> and prints one JSON line of what it found:
// { site, shown, inside, overlaps, small, steer, gas, both, released, tap, scroll, zoom, portrait, keyboard, again }.
import http from 'node:http';
import fs from 'node:fs';
import path from 'node:path';
import { spawn } from 'node:child_process';

const [webdir, out, profile] = process.argv.slice(2);
const types = { '.html': 'text/html', '.js': 'text/javascript', '.wasm': 'application/wasm', '.svg': 'image/svg+xml',
  '.webmanifest': 'application/manifest+json', '.ttf': 'font/ttf', '.png': 'image/png' };
const server = http.createServer((req, res) => {
  let p = decodeURIComponent(new URL(req.url, 'http://x').pathname);
  if (p.endsWith('/')) p += 'index.html';
  fs.readFile(path.join(webdir, path.normalize(p)), (e, b) => {
    if (e) {
      res.writeHead(404);
      res.end();
      return;
    }
    res.writeHead(200, { 'content-type': types[path.extname(p)] ?? 'application/octet-stream' });
    res.end(b);
  });
}).listen(0, '127.0.0.1');
await new Promise(r => server.on('listening', r));
const url = `http://127.0.0.1:${server.address().port}/`;
fs.mkdirSync(out, { recursive: true });

const chrome = spawn('chromium', ['--headless=new', '--remote-debugging-port=0', `--user-data-dir=${profile}`,
  '--no-first-run', '--no-default-browser-check', '--hide-scrollbars', 'about:blank'], { stdio: ['ignore', 'ignore', 'pipe'] });
const fail = why => {
  console.error(why);
  chrome.kill();
  process.exit(1);
};
process.on('uncaughtException', e => fail(e.stack ?? String(e)));
const wsUrl = await new Promise(resolve => {
  let buf = '';
  const late = setTimeout(() => fail(`chromium gave no DevTools address:\n${buf}`), 20000);
  chrome.stderr.on('data', d => {
    buf += d;
    const m = buf.match(/DevTools listening on (ws:\S+)/);
    if (m) {
      clearTimeout(late);
      resolve(m[1]);
    }
  });
});
const targets = await (await fetch(wsUrl.replace('ws://', 'http://').replace(/\/devtools.*/, '/json'))).json();
const ws = new WebSocket(targets.find(t => t.type === 'page').webSocketDebuggerUrl);
await new Promise(r => ws.addEventListener('open', r));
let id = 0;
const pending = new Map();
ws.addEventListener('message', ({ data }) => {
  const m = JSON.parse(data);
  if (m.id && pending.has(m.id)) {
    pending.get(m.id)(m);
    pending.delete(m.id);
  } else if (m.method === 'Runtime.exceptionThrown') console.error('page:', JSON.stringify(m.params.exceptionDetails));
});
const send = (method, params = {}) => new Promise(r => {
  const i = ++id;
  pending.set(i, r);
  ws.send(JSON.stringify({ id: i, method, params }));
});
const evaluate = async expression =>
  (await send('Runtime.evaluate', { expression, returnByValue: true, awaitPromise: true })).result?.result?.value;
// two of the page's frames: a button let go is up once a poll has seen it and a later frame started
const frames = () => evaluate('new Promise(r => requestAnimationFrame(() => requestAnimationFrame(() => r(true))))');
// the touch pad as the worker gets it: index.html polls `getGamepads()` once a frame
const pad = () => evaluate(`(() => { const g = [...navigator.getGamepads()].find(g => g?.index === 16);
  return g ? { axes: g.axes.slice(0, 2), down: g.buttons.map((b, i) => b.pressed ? i : -1).filter(i => i >= 0) } : null; })()`);
const touch = (type, points) => send('Input.dispatchTouchEvent', { type, touchPoints: points });
const centre = sel => evaluate(`(() => { const r = document.querySelector('${sel}').getBoundingClientRect();
  return { x: r.left + r.width / 2, y: r.top + r.height / 2, w: r.width }; })()`);
const size = async (width, height) => {
  await send('Emulation.setDeviceMetricsOverride', { width, height, deviceScaleFactor: 1, mobile: true,
    screenOrientation: width > height ? { type: 'landscapePrimary', angle: 90 } : { type: 'portraitPrimary', angle: 0 } });
  await frames();
};
const shot = async name => {
  const png = await send('Page.captureScreenshot', { format: 'png' });
  fs.writeFileSync(path.join(out, name), Buffer.from(png.result.data, 'base64'));
};

await send('Runtime.enable');
await send('Page.enable');
await send('Emulation.setTouchEmulationEnabled', { enabled: true, maxTouchPoints: 5 });
// an iPhone 13's screen on its side
await size(844, 390);
await send('Page.navigate', { url });
for (let t = Date.now(); !['asking', 'menu'].includes(await evaluate('document.body.dataset.state ?? ""')); ) {
  if (Date.now() - t > 20000) fail('the page never came up');
  await new Promise(r => setTimeout(r, 100));
}
const found = {};
await frames();
// on the site (no game yet) there is no pad
found.site = await evaluate('window.openmxTouch?.shown() ?? null');
// the game "starts": the page's own flag, which index.html sets on the worker's `started`
await evaluate(`document.body.dataset.state = 'started'`);
await frames();
found.shown = await evaluate('window.openmxTouch.shown()');
// every control inside the screen, none on another, none smaller than a fingertip (40 css px)
Object.assign(found, await evaluate(`(() => {
  const rs = [...document.querySelectorAll('#touch button')].map(b => b.getBoundingClientRect());
  const inside = rs.every(r => r.left >= 0 && r.top >= 0 && r.right <= innerWidth && r.bottom <= innerHeight);
  let overlaps = 0;
  for (let i = 0; i < rs.length; i++) for (let j = i + 1; j < rs.length; j++) {
    const a = rs[i], b = rs[j];
    if (a.left < b.right && b.left < a.right && a.top < b.bottom && b.top < a.bottom) overlaps++;
  }
  return { inside, overlaps, small: rs.filter(r => Math.min(r.width, r.height) < 33).length, buttons: rs.length };
})()`));
await shot('landscape.png');

// the left thumb down in the stick's area and pushed full right and a little down; the right thumb on GAS
const gas = await centre('[data-b="r2"]');
const knob = await centre('#touch .base');
const at = { x: 140, y: 280 };
await touch('touchStart', [{ x: at.x, y: at.y, id: 1 }]);
await touch('touchMove', [{ x: at.x + knob.w, y: at.y + knob.w / 4, id: 1 }]);
await frames();
found.steer = await pad();
await touch('touchStart', [{ x: at.x + knob.w, y: at.y + knob.w / 4, id: 1 }, { x: gas.x, y: gas.y, id: 2 }]);
await frames();
found.gas = await pad();
// the brake beside it, a third finger
const brake = await centre('[data-b="l2"]');
await touch('touchStart', [{ x: at.x + knob.w, y: at.y + knob.w / 4, id: 1 }, { x: gas.x, y: gas.y, id: 2 }, { x: brake.x, y: brake.y, id: 3 }]);
await frames();
found.both = await pad();
await touch('touchEnd', []);
await frames();
await frames();
found.released = await pad();

// a quick tap in the stick's area is the menus' click, handed to the canvas
await evaluate(`window.taps = 0; document.getElementById('screen').addEventListener('pointerdown', () => taps++)`);
await touch('touchStart', [{ x: 200, y: 250, id: 4 }]);
await touch('touchEnd', []);
await frames();
found.tap = await evaluate('taps');
// a drag on the canvas (right of the stick, above the buttons) scrolls nothing; the viewport refuses zoom
const free = await centre('[data-b="select"]');
await touch('touchStart', [{ x: free.x, y: free.y + 80, id: 5 }]);
for (let i = 1; i <= 5; i++) await touch('touchMove', [{ x: free.x - 10 * i, y: free.y + 80 - 30 * i, id: 5 }]);
await touch('touchEnd', []);
await frames();
found.scroll = await evaluate('[scrollX, scrollY, document.scrollingElement.scrollWidth - innerWidth, document.scrollingElement.scrollHeight - innerHeight]');
found.zoom = await evaluate(`document.querySelector('meta[name=viewport]').content.includes('user-scalable=no')`);

// upright: the pad goes and the hint to turn the phone covers the game
await size(390, 844);
found.portrait = await evaluate(`({ pad: getComputedStyle(document.getElementById('touch')).display,
  hint: getComputedStyle(document.querySelector('.turn')).display, text: document.querySelector('.turn').textContent.trim() })`);
await shot('portrait.png');
await size(844, 390);

// a hardware key hides it; the next touch brings it back
await send('Input.dispatchKeyEvent', { type: 'keyDown', key: 'ArrowUp', code: 'ArrowUp', windowsVirtualKeyCode: 38 });
await send('Input.dispatchKeyEvent', { type: 'keyUp', key: 'ArrowUp', code: 'ArrowUp', windowsVirtualKeyCode: 38 });
await frames();
found.keyboard = await evaluate('window.openmxTouch.shown()');
await touch('touchStart', [{ x: 600, y: 120, id: 6 }]);
await touch('touchEnd', []);
await frames();
found.again = await evaluate('window.openmxTouch.shown()');

console.log(JSON.stringify(found));
chrome.kill();
await new Promise(r => chrome.on('exit', r));
server.close();
process.exit(0);
