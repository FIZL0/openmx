// A pad through a browser's own Gamepad API (P245; crates/openmx/tests/web.rs, `a_firefox_pad_rides`): beside P236's
// gamepad.mjs, whose pad is a script standing in for chromium's `getGamepads()`, this one is Firefox's. Firefox's test
// service (`navigator.requestGamepadServiceTest()`, behind dom.gamepad.test.enabled, what its own mochitests use) adds
// a pad to the parent process's gamepad service as a platform backend does; from there it is Firefox's code that
// lists it in `getGamepads()`, fires `gamepadconnected` and snapshots its buttons, as with a USB pad. No kernel pad:
// /dev/uinput is root's on the machine the tests run on, and one would reach every other slot's browser (K13).
//
//   node firefox_gamepad.mjs <firefox> <web dir> <disc.iso> <scratch profile dir> <out.png>
//
// <firefox> is tools/firefox.sh's. Firefox gives the service only to a browser "in automation": the profile's
// security.turn_off_all_security_so_that_viruses_can_take_over_this_computer and MOZ_DISABLE_NONLOCAL_CONNECTIONS=1,
// under which any connection off the machine aborts the browser, so the profile turns off its update, telemetry and
// remote-settings services and sends what is left to a dead proxy on loopback. Driven over WebDriver BiDi on node's
// own WebSocket. With OPENMX_GPU_PKG set the page's pkg/ is served from there.
//
// The page opened as a player opens it, the disc picked; on the splash the pad is plugged in (`addGamepad`, standard
// mapping) and a second later its Cross pressed, which must make it Player 1 (`pads: 1P: … the menus and the first
// seat are its`). Then Cross until a ride seats it, and the ride is watched on its L2 and then its R2: the speedometer
// still over two seconds of brake, as gamepad.mjs, and the gas read against that braked picture (its 0) 3.5 s in, since
// in software the ride is slow and two pictures on the gas can show one speed. Prints one JSON line:
// { state, gpu, listed, first, taps, seated, braked, gas, png } — `listed` whether `getGamepads()` showed the pad
// before any press (Firefox holds a pad back until one, as chromium does), the rest as gamepad.mjs.
import http from 'node:http';
import fs from 'node:fs';
import path from 'node:path';
import { spawn } from 'node:child_process';

const [firefox, webdir, iso, profile, out] = process.argv.slice(2);
const PKG = process.env.OPENMX_GPU_PKG;
const PAD = 'Firefox test pad (STANDARD GAMEPAD)';
const said = [];
const types = { '.html': 'text/html', '.js': 'text/javascript', '.wasm': 'application/wasm', '.svg': 'image/svg+xml',
  '.webmanifest': 'application/manifest+json', '.ttf': 'font/ttf' };
const server = http.createServer((req, res) => {
  let p = decodeURIComponent(new URL(req.url, 'http://x').pathname);
  if (p.endsWith('/')) p += 'index.html';
  const f = PKG && p.startsWith('/pkg/') ? path.join(PKG, path.normalize(p.slice(5))) : path.join(webdir, path.normalize(p));
  fs.readFile(f, (e, b) => {
    if (e) {
      res.writeHead(404);
      res.end();
      return;
    }
    // OPFS refused, as gamepad.mjs: the picked file is read where it is, and nothing is copied
    if (p === '/index.html') b = Buffer.from(String(b).replace('opfs = true;', 'opfs = false;'));
    res.writeHead(200, { 'content-type': types[path.extname(f)] ?? 'application/octet-stream' });
    res.end(b);
  });
}).listen(0, '127.0.0.1');
await new Promise(r => server.on('listening', r));
const url = `http://127.0.0.1:${server.address().port}/`;

const prefs = {
  'dom.gamepad.test.enabled': true,
  'security.turn_off_all_security_so_that_viruses_can_take_over_this_computer': true,
  'remote.active-protocols': 1,
  // everything but loopback to a port nothing answers on, so no connection leaves the machine (see the top)
  'network.proxy.type': 1,
  'network.proxy.http': '127.0.0.1',
  'network.proxy.http_port': 9,
  'network.proxy.ssl': '127.0.0.1',
  'network.proxy.ssl_port': 9,
  'network.proxy.share_proxy_settings': true,
  'network.proxy.no_proxies_on': '',
  'network.proxy.allow_hijacking_localhost': false,
  // a dead proxy is otherwise passed by, straight to the address
  'network.proxy.failover_direct': false,
  'app.update.disabledForTesting': true,
  'app.update.auto': false,
  'app.normandy.enabled': false,
  'services.settings.server': 'http://127.0.0.1:9/',
  'browser.safebrowsing.malware.enabled': false,
  'browser.safebrowsing.phishing.enabled': false,
  'browser.safebrowsing.downloads.enabled': false,
  'browser.safebrowsing.update.enabled': false,
  'extensions.update.enabled': false,
  'extensions.getAddons.cache.enabled': false,
  'datareporting.policy.dataSubmissionEnabled': false,
  'datareporting.healthreport.uploadEnabled': false,
  'toolkit.telemetry.enabled': false,
  'toolkit.telemetry.server': '',
  'network.captive-portal-service.enabled': false,
  'network.connectivity-service.enabled': false,
  'network.dns.disablePrefetch': true,
  'network.prefetch-next': false,
  'browser.region.network.url': '',
  'browser.search.update': false,
  'media.gmp-manager.url': '',
  'media.gmp-manager.updateEnabled': false,
  'media.gmp-provider.enabled': false,
  'media.gmp-widevinecdm.enabled': false,
  'media.gmp-gmpopenh264.enabled': false,
  'media.eme.enabled': false,
  'browser.startup.page': 0,
  'browser.startup.homepage_override.mstone': 'ignore',
  'browser.newtabpage.enabled': false,
  'browser.shell.checkDefaultBrowser': false,
  // No dom.storageManager.prompt.testing: index.html's `navigator.storage.persist()` stays an unanswered prompt,
  // and the ride must start anyway (B14).
  // the headless window's size, as gamepad.mjs's chromium (the speedometer crop is a share of it)
  'privacy.window.maxInnerWidth': 1280,
  'privacy.window.maxInnerHeight': 720,
};
fs.mkdirSync(profile, { recursive: true });
fs.writeFileSync(path.join(profile, 'user.js'),
  Object.entries(prefs).map(([k, v]) => `user_pref(${JSON.stringify(k)}, ${JSON.stringify(v)});\n`).join(''));
const fox = spawn(firefox, ['--headless', '--no-remote', '--profile', profile, '--remote-debugging-port=0',
  '--width=1280', '--height=720', 'about:blank'], {
  stdio: ['ignore', 'pipe', 'pipe'],
  env: { ...process.env, MOZ_DISABLE_NONLOCAL_CONNECTIONS: '1', MOZ_CRASHREPORTER_DISABLE: '1' },
});
let foxSaid = '';
let done = false;
const exited = new Promise(r => fox.on('exit', r));
fox.on('exit', code => {
  if (!done) fail(`firefox exited (${code}) mid-test:\n${foxSaid.slice(-4000)}`);
});
const fail = why => {
  console.error(why);
  done = true;
  fox.kill();
  process.exit(1);
};
const wsUrl = await new Promise(resolve => {
  const late = setTimeout(() => fail(`firefox gave no WebDriver BiDi address:\n${foxSaid}`), 30000);
  const on = d => {
    foxSaid += d;
    const m = foxSaid.match(/WebDriver BiDi listening on (ws:\S+)/);
    if (m) {
      clearTimeout(late);
      resolve(m[1]);
    }
  };
  fox.stderr.on('data', on);
  fox.stdout.on('data', on);
});
const ws = new WebSocket(`${wsUrl}/session`);
await new Promise(r => ws.addEventListener('open', r));
let id = 0;
const pending = new Map();
ws.addEventListener('message', ({ data }) => {
  const m = JSON.parse(data);
  if (m.id && pending.has(m.id)) {
    pending.get(m.id)(m);
    pending.delete(m.id);
  } else if (m.method === 'log.entryAdded') {
    // the page's and its worker's console both
    const line = m.params.type === 'console' ? m.params.args.map(a => a.value ?? a.type).join(' ') : m.params.text;
    said.push(line);
    console.error('console:', line);
  }
});
const send = (method, params = {}) => new Promise(r => {
  const i = ++id;
  pending.set(i, r);
  ws.send(JSON.stringify({ id: i, method, params }));
});
const ok = async (method, params) => {
  const m = await send(method, params);
  if (m.type !== 'success') fail(`${method}: ${JSON.stringify(m)}`);
  return m.result;
};
await ok('session.new', { capabilities: {} });
await ok('session.subscribe', { events: ['log.entryAdded'] });
const context = (await ok('browsingContext.getTree', {})).contexts[0].context;
const evaluate = async expression => {
  const m = await send('script.evaluate', { expression, target: { context }, awaitPromise: true });
  return m.result?.type === 'success' ? m.result.result?.value : undefined;
};
// BiDi hands objects back as lists of [key, value] pairs: the dataset comes as JSON instead
const data = async () => JSON.parse((await evaluate('JSON.stringify({ ...document.body.dataset })')) ?? '{}');
const wait = ms => new Promise(r => setTimeout(r, ms));
const until = async (good, ms) => {
  const t = Date.now();
  for (;;) {
    const d = await data();
    if (good(d) || Date.now() - t > ms) return d;
    await wait(100);
  }
};

await ok('browsingContext.navigate', { context, url, wait: 'complete' });
let d = await until(d => ['asking', 'failed'].includes(d.state), 60000);
if (d.state !== 'asking') fail(`the page did not ask for the disc: ${JSON.stringify(d)}`);
const pick = await send('script.evaluate', { expression: "document.querySelector('#pick')", target: { context },
  awaitPromise: false, resultOwnership: 'root' });
await ok('input.setFiles', { context, element: { sharedId: pick.result.result.sharedId }, files: [iso] });
d = await until(d => d.state === 'failed' || d.drawn || (d.state === 'started' && d.gpu === 'software'), 180000);
if (d.state === 'failed' || !(d.drawn || d.gpu === 'software'))
  fail(`the page did not start drawing: ${JSON.stringify(d)} (${await evaluate("document.querySelector('#status')?.textContent")})`);

// a line the console said since `from`, waited for up to `ms`
const heard = async (from, test, ms) => {
  const t = Date.now();
  for (;;) {
    if (said.slice(from).some(test)) return true;
    if (Date.now() - t > ms) return false;
    await wait(50);
  }
};
// The pad's buttons go through the service; `pad` is the index addGamepad gave.
const press = (b, v) => evaluate(`openmxTestService.newButtonValueEvent(openmxTestIndex, ${b}, ${v > 0.5}, ${v > 0}, ${v}), true`);
const tap = async b => {
  await press(b, 1);
  await wait(100);
  await press(b, 0);
};
if (!(await heard(0, l => l.startsWith('music: playing'), 60000))) fail('no song started: the front end never came up');
await wait(2000);
// The pad plugged in: 17 buttons, 4 axes, the standard mapping, no hand, no haptics, lights or touch.
const added = await evaluate(`(async () => {
  window.openmxTestService = navigator.requestGamepadServiceTest();
  window.openmxTestIndex = await openmxTestService.addGamepad(${JSON.stringify(PAD)}, openmxTestService.standardMapping,
    openmxTestService.noHand, 17, 4, 0, 0, 0);
  return openmxTestIndex;
})()`);
if (typeof added !== 'number') fail(`the test service added no pad (${added}): is dom.gamepad.test.enabled read?`);
await wait(1000);
const listed = await evaluate('navigator.getGamepads().some(g => g)');
let from = said.length;
await tap(0);
const menus = l => l.startsWith(`pads: 1P: ${PAD}`) && l.includes('the menus');
const first = await heard(from, menus, 3000);
const seated = l => l.startsWith(`pads: 1P: ${PAD}`) && !l.includes('the menus');
let taps = 1;
from = said.length;
while (!(await heard(from, seated, 1500)) && taps < 12) {
  await tap(0);
  taps++;
}
const ride = said.slice(from).some(seated);
// The speedometer, as gamepad.mjs reads it.
const shot = async () => (await ok('browsingContext.captureScreenshot', { context })).data;
const change = (a, b) => evaluate(`(async () => {
  const read = async s => {
    const i = await createImageBitmap(await (await fetch('data:image/png;base64,' + s)).blob());
    const [x, y, w, h] = [Math.round(i.width * 0.84), Math.round(i.height * 0.70), Math.round(i.width * 0.12), Math.round(i.height * 0.18)];
    const c = new OffscreenCanvas(w, h);
    const g = c.getContext('2d');
    g.drawImage(i, x, y, w, h, 0, 0, w, h);
    return g.getImageData(0, 0, w, h).data;
  };
  const [p, q] = [await read('${a}'), await read('${b}')];
  let moved = 0;
  for (let k = 0; k < p.length; k += 4)
    if (Math.abs(p[k] - q[k]) + Math.abs(p[k + 1] - q[k + 1]) + Math.abs(p[k + 2] - q[k + 2]) > 96) moved++;
  return moved / (p.length / 4);
})()`);
let braked = null;
let gas = null;
if (ride) {
  await press(6, 1);
  await wait(5000);
  const a = await shot();
  await wait(2000);
  const b = await shot();
  braked = await change(a, b);
  await press(6, 0);
  await press(7, 1);
  await wait(3500);
  const e = await shot();
  gas = await change(b, e);
  fs.writeFileSync(out.replace(/\.png$/, '-braked.png'), Buffer.from(b, 'base64'));
  fs.writeFileSync(out, Buffer.from(e, 'base64'));
}
d = await data();
console.log(JSON.stringify({ state: d.state, gpu: d.gpu, listed, first, taps, seated: ride, braked, gas, png: out }));
done = true;
fox.kill();
await exited;
server.close();
process.exit(0);
