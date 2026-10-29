// The game's thread (N4): the user's disc, the game module, and the canvas it draws in.
//
// The disc is read synchronously, a sector run at a time, because the shell reads it that way on the desktop
// (crates/game-render/src/disc.rs) and a wasm call cannot wait on a promise. Workers have two ways to do that:
//   * OPFS `disc.iso` through a FileSystemSyncAccessHandle — the copy index.html makes once, so the next visit
//     starts without asking for the disc again;
//   * the picked File itself through FileReaderSync, for a browser that refuses OPFS (a private window): nothing is
//     copied and the disc is asked for again next visit.
// The module's host calls (crates/openmx/src/web.rs says what each one is) are answered below, on the global
// `openmx` wasm-bindgen's glue calls them on; that list and crates/openmx/tests/web/host.mjs, which answers them in
// node for the tests, are kept call for call alike. A module worker: the glue is an ES module.
//
// The game (P196): `{ start }` runs the desktop's own front end — splash, main menu, and the rides, lobby, settings
// and mods it starts — a frame each time this thread's animation frame comes round (`frame`), until the shell quits.
// The page sends what it has that a worker cannot read: keys (X keycodes), pointer presses, the gamepads, the canvas's
// size; the mixer's sound goes back to it to play. The shell's own files (settings, controls, picks, markers) are
// OPFS's `files/` folder, each read before the game starts and written back as it changes.
//
// Online (N4): `{ online }` joins or hosts a lobby through a relay and rides once the host starts, a frame per
// tick of the ride's 30 Hz clock on this thread's timer; the page's seat rides a pad nobody holds (web.rs says why). It needs no disc.
//
// The picture (P48): the canvas goes to the module, whose GPU draws on it — WebGPU, WebGL2, as `api` asks (`auto`,
// `webgl2`, `software`) — and says which with `gpu`, then `drawn` at its first frame. Only the software renderer's
// frames come through `present`, which is the only thing that takes the canvas's 2D context: a canvas has one kind of
// context for good. The page restarts on the next API down when the GPU fails before its first frame.
//
// Messages in:  { import: File, canvas?, api?, args?, phone? }   copy the disc into OPFS, then start
//               { start: { canvas?, file?, api?, args?, phone? } } start on the OPFS copy, or on `file` when given
//               { online: { name, relay, codes?, join?, host?, bike, ticks? } }
//               { key: code << 1 | down } | { blur } | { click: [x, y] } | { view: [w, h] } | { pads, names? }
//               { played: frames }
// Messages out: { progress: [done, total] } | { started: true } | { error: String } | { missing: true }
//               { gpu: api } (what the module draws with) | { drawn: [api, w, h] } (its first GPU frame)
//               { fps: number } (pictures in the last second)
//               { play: Float32Array } (the mixer's next stereo frames) | { quit: String } (the shell ended)
//               { online: state } (Online.state(), a few times a second) | { ticks: String } (once `ticks` settled)

const DISC = 'disc.iso';
// What one read of the picked file asks for when it is being copied: big enough that a 2 GB image is a few hundred
// reads, small enough to hold in memory twice.
const CHUNK = 8 << 20;

let canvas;
// The 2D context, taken at the software renderer's first frame (P48); null when the GPU has the canvas.
let context;
// Pictures since `counted`, the page told each second.
let pictures = 0;
let counted = 0;
// What the host calls read: the disc once there is one.
let disc = { size: 0, read: () => -1 };
// What the page has said since the shell last asked.
let keys = [];
const heldKeys = new Set();
let clicks = [];
let view = [640, 448];
let pads = [];
let padNames = {};
// Stereo frames handed to the page and not played yet.
let queued = 0;
// The shell's files, by path ("settings.txt", "markers/…"), read before it starts; each change goes back to OPFS in turn.
const files = new Map();
let saving = Promise.resolve();
const FILES = 'files';
const fileKey = path => path.replace(/^\/opfs\//, '').replace(/^\.\//, '');

globalThis.openmx = {
  disc_size: () => disc.size,
  disc_read: (at, into) => {
    try {
      return disc.read(at, into);
    } catch {
      return -1;
    }
  },
  present: (rgba, w, h) => {
    if (!canvas) return;
    context ??= canvas.getContext('2d');
    if (!context) return;
    if (canvas.width !== w || canvas.height !== h) [canvas.width, canvas.height] = [w, h];
    context.putImageData(new ImageData(new Uint8ClampedArray(rgba), w, h), 0, 0);
  },
  gpu: api => postMessage({ gpu: api }),
  drawn: (api, w, h) => postMessage({ drawn: [api, w, h] }),
  log: line => console.log(line),
  view: () => Uint32Array.from(view),
  keys: () => Uint32Array.from(keys.splice(0)),
  clicks: () => Int32Array.from(clicks.splice(0)),
  gamepads: () => Float32Array.from(pads),
  gamepad_name: i => padNames[i] ?? '',
  play: samples => {
    queued += samples.length / 2;
    const copy = Float32Array.from(samples);
    postMessage({ play: copy }, [copy.buffer]);
  },
  queued: () => queued,
  file_read: path => files.get(fileKey(path)),
  file_write: (path, bytes) => {
    const key = fileKey(path);
    if (bytes) files.set(key, Uint8Array.from(bytes));
    else files.delete(key);
    saving = saving.then(() => store(key, bytes)).catch(e => console.log(`openmx: ${key}: ${e}`));
  },
  frame: () => {
    pictures++;
    const now = performance.now();
    if (now - counted >= 1000) {
      if (counted) postMessage({ fps: (1000 * pictures) / (now - counted) });
      pictures = 0;
      counted = now;
    }
    return new Promise(r => (self.requestAnimationFrame ?? (f => setTimeout(f, 16)))(r));
  },
};

// One of the shell's files into OPFS (`files/`, its path as one name), or out of it.
async function store(key, bytes) {
  const dir = await (await opfs()).getDirectoryHandle(FILES, { create: true });
  const name = encodeURIComponent(key);
  if (!bytes) {
    await dir.removeEntry(name).catch(() => {});
    return;
  }
  const h = await (await dir.getFileHandle(name, { create: true })).createSyncAccessHandle();
  try {
    h.truncate(0);
    h.write(bytes, { at: 0 });
    h.flush();
  } finally {
    h.close();
  }
}

// Every one of the shell's files OPFS has, before the shell reads any.
async function loadFiles() {
  try {
    const dir = await (await opfs()).getDirectoryHandle(FILES, { create: true });
    for await (const [name, handle] of dir.entries()) {
      if (handle.kind === 'file') files.set(decodeURIComponent(name), new Uint8Array(await (await handle.getFile()).arrayBuffer()));
    }
  } catch {
    // no OPFS (a private window): the shell starts on its defaults and keeps nothing
  }
}

// The module, once: wasm-bindgen's glue fetches pkg/openmx_web_bg.wasm beside itself.
let loaded;
function game() {
  loaded ??= import('./pkg/openmx_web.js').then(async glue => {
    await glue.default();
    return glue;
  });
  return loaded;
}

async function opfs() {
  return navigator.storage.getDirectory();
}

// A sync access handle holds its file's lock, one per file. WebKit refuses a second with InvalidStateError (others
// say NoModificationAllowedError): an iOS home-screen app reloaded under an update whose old worker still holds the
// disc, or a second tab. `held(f)` gives such a holder ~3 s to let go, then throws.
const LOCKED = ['InvalidStateError', 'NoModificationAllowedError'];
async function held(f) {
  for (let tries = 0; ; tries++) {
    try {
      return await f();
    } catch (e) {
      if (tries >= 10 || !LOCKED.includes(e?.name)) throw e;
      await new Promise(go => setTimeout(go, 300));
    }
  }
}

// The picked disc into OPFS, under a temporary name first so a copy cut short is never taken for a disc.
async function importDisc(file) {
  // the copy about to be replaced lets go of its lock first
  disc.close?.();
  disc = { size: 0, read: () => -1 };
  const dir = await opfs();
  const part = await dir.getFileHandle(`${DISC}.part`, { create: true });
  const h = await held(() => part.createSyncAccessHandle());
  try {
    h.truncate(0);
    for (let at = 0; at < file.size; at += CHUNK) {
      const bytes = new Uint8Array(await file.slice(at, at + CHUNK).arrayBuffer());
      for (let done = 0; done < bytes.length;) {
        const n = h.write(bytes.subarray(done), { at: at + done });
        if (!(n > 0)) throw new Error(`wrote ${at + done} of ${file.size} bytes`);
        done += n;
      }
      postMessage({ progress: [Math.min(at + CHUNK, file.size), file.size] });
    }
    h.flush();
  } finally {
    h.close();
  }
  // (directory, name): WebKit's move takes no name-only form
  if (part.move) await held(() => part.move(dir, DISC));
  else {
    const out = await (await dir.getFileHandle(DISC, { create: true })).createWritable();
    await out.write(await part.getFile());
    await out.close();
    await dir.removeEntry(`${DISC}.part`);
  }
}

// A reader over the disc: `size` and `read(at, into)`, the bytes read.
async function openDisc(file) {
  if (file) {
    const sync = new FileReaderSync();
    return {
      size: file.size,
      read: (at, into) => {
        const bytes = new Uint8Array(sync.readAsArrayBuffer(file.slice(at, at + into.length)));
        into.set(bytes);
        return bytes.length;
      },
    };
  }
  // this worker has the copy open already (Ride pressed again): its lock is ours, so keep it
  if (disc.opfs) return disc;
  let handle;
  try {
    handle = await (await opfs()).getFileHandle(DISC);
  } catch {
    return null;
  }
  try {
    const h = await held(() => handle.createSyncAccessHandle());
    return { size: h.getSize(), read: (at, into) => h.read(into, { at }), opfs: true, close: () => h.close() };
  } catch (e) {
    if (!LOCKED.includes(e?.name)) throw e;
    // still held elsewhere: the copy's File reads without a lock
    console.log(`disc: ${e.name} on the sync handle, reading the copy's File instead`);
    return { ...(await openDisc(await handle.getFile())), opfs: true };
  }
}

async function start(file, api, args, phone) {
  const opened = await openDisc(file);
  if (!opened) {
    postMessage({ missing: true });
    return;
  }
  disc = opened;
  await loadFiles();
  const glue = await game();
  glue.openmx_phone(!!phone);
  postMessage({ started: true });
  const ended = await glue.openmx_run(canvas, api || 'auto', args || '');
  await saving;
  if (ended) postMessage({ error: ended });
  else postMessage({ quit: '' });
}

// A ride against other machines: join (a room code or ticket) or host (that many seats), then a frame every tick of
// the ride's 30 Hz clock (1001/30 ms, N6), ready as soon as seated; a host starts once every seat is taken and ready. `ticks`: post
// the first that many ticks' lines once settled.
async function online({ name, relay, codes, join, host, bike, ticks }) {
  const glue = await game();
  const net = new glue.Online(name || 'Browser', relay || '', codes ?? undefined);
  if (join) net.join(join, bike || 'race125p');
  else net.host(Number(host) || 2, bike || 'race125p');
  let last = performance.now();
  let told = 0;
  let readied = false;
  let sent = false;
  let started = false;
  setInterval(() => {
    const now = performance.now();
    net.frame(Math.min((now - last) / 1000, 0.25));
    last = now;
    const s = net.state();
    if (join && !readied && s.taken > 1) {
      net.ready(true);
      readied = true;
    }
    // hosting: start as soon as every seat is taken and every guest is ready
    if (host && !started && s.taken >= Number(host) && s.ready) {
      net.start();
      started = true;
    }
    // settled, not confirmed: a tick confirmed between runs can still hold its prediction (K7)
    const lines = ticks && !sent && s.settled >= Number(ticks) ? net.ticks(0, Number(ticks)) : '';
    if (lines && lines.split('\n').filter(Boolean).length === Number(ticks)) {
      postMessage({ ticks: lines });
      sent = true;
    }
    if (now - told > 250) {
      postMessage({ online: s });
      told = now;
    }
  }, 1001 / 30);
}

// A panic traps out of the module's task queue: the page hears of it as an error, as it does a refused promise.
addEventListener('unhandledrejection', e => postMessage({ error: String(e.reason) }));

onmessage = async ({ data }) => {
  try {
    // The canvas comes once, with whichever comes first of a start and an import (index.html sends it when the menu's
    // Ride is pressed or a disc is picked); a later start (the picked file) draws in the same one.
    const given = data.start?.canvas ?? data.canvas;
    if (given) canvas = given;
    if (data.key !== undefined) {
      keys.push(data.key);
      if (data.key & 1) heldKeys.add(data.key >> 1);
      else heldKeys.delete(data.key >> 1);
    } else if (data.blur) {
      for (const k of heldKeys) keys.push(k << 1);
      heldKeys.clear();
    } else if (data.click) {
      clicks.push(...data.click);
    } else if (data.view) {
      view = data.view;
    } else if (data.pads) {
      pads = data.pads;
      if (data.names) padNames = data.names;
    } else if (data.played) {
      queued = Math.max(0, queued - data.played);
    } else if (data.start) {
      await start(data.start.file, data.start.api, data.start.args, data.start.phone);
    } else if (data.online) {
      await online(data.online);
    } else if (data.import) {
      await importDisc(data.import);
      await start(undefined, data.api, data.args, data.phone);
    }
  } catch (e) {
    postMessage({
      error: LOCKED.includes(e?.name)
        ? `The disc is busy in another openMX window or tab: close the others, then reload. (${e.name})`
        : String(e),
    });
  }
};
