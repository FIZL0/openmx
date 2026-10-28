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
// Online (N4): `{ online }` joins or hosts a lobby through a relay and rides once the host starts, a frame per
// tick of the ride's 30 Hz clock on this thread's timer; the page's seat rides a pad nobody holds (web.rs says why). It needs no disc.
//
// Messages in:  { import: File, canvas? }   copy the disc into OPFS, then start
//               { start: { canvas?, file? } } start on the OPFS copy, or on `file` when given
//               { online: { name, relay, codes?, join?, host?, bike, ticks? } }
// Messages out: { progress: [done, total] } | { started: true } | { error: String } | { missing: true }
//               { online: state } (Online.state(), a few times a second) | { ticks: String } (once `ticks` settled)

const DISC = 'disc.iso';
// What one read of the picked file asks for when it is being copied: big enough that a 2 GB image is a few hundred
// reads, small enough to hold in memory twice.
const CHUNK = 8 << 20;

let canvas;
let context;
// What the host calls read: the disc once there is one.
let disc = { size: 0, read: () => -1 };

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
    if (canvas.width !== w || canvas.height !== h) [canvas.width, canvas.height] = [w, h];
    context.putImageData(new ImageData(new Uint8ClampedArray(rgba), w, h), 0, 0);
  },
  log: line => console.log(line),
};

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

async function start(file) {
  const opened = await openDisc(file);
  if (!opened) {
    postMessage({ missing: true });
    return;
  }
  disc = opened;
  const glue = await game();
  const err = glue.openmx_start(canvas.width, canvas.height);
  if (err) {
    postMessage({ error: err });
    return;
  }
  glue.openmx_frame();
  postMessage({ started: true });
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
    // settled can be ahead of the ticks run here: wait for every line
    const lines = ticks && !sent && s.confirmed >= Number(ticks) ? net.ticks(0, Number(ticks)) : '';
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

onmessage = async ({ data }) => {
  try {
    // The canvas comes once, with whichever comes first of a start and an import (index.html sends it when the menu's
    // Ride is pressed or a disc is picked); a later start (the picked file) draws in the same one.
    const given = data.start?.canvas ?? data.canvas;
    if (given) {
      canvas = given;
      context = canvas.getContext('2d');
    }
    if (data.start) {
      await start(data.start.file);
    } else if (data.online) {
      await online(data.online);
    } else if (data.import) {
      await importDisc(data.import);
      await start();
    }
  } catch (e) {
    postMessage({
      error: LOCKED.includes(e?.name)
        ? `The disc is busy in another openMX window or tab: close the others, then reload. (${e.name})`
        : String(e),
    });
  }
};
