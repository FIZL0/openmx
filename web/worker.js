// The game's thread (N4): the user's disc, the game module, and the canvas it draws in.
//
// The disc is read synchronously, a sector run at a time, because the shell reads it that way on the desktop
// (crates/game-render/src/disc.rs) and a wasm call cannot wait on a promise. Workers have two ways to do that:
//   * OPFS `disc.iso` through a FileSystemSyncAccessHandle — the copy index.html makes once, so the next visit
//     starts without asking for the disc again;
//   * the picked File itself through FileReaderSync, for a browser that refuses OPFS (a private window): nothing is
//     copied and the disc is asked for again next visit.
// The module's host calls (crates/openmx/src/web.rs says what each one is) are answered below; that list and
// crates/openmx/tests/web/host.mjs, which answers them in node for the tests, are kept call for call alike.
//
// Messages in:  { import: File }            copy the disc into OPFS, then start
//               { start: { canvas?, file? } } start on the OPFS copy, or on `file` when given
// Messages out: { progress: [done, total] } | { started: true } | { error: String } | { missing: true }

const DISC = 'disc.iso';
// What one read of the picked file asks for when it is being copied: big enough that a 2 GB image is a few hundred
// reads, small enough to hold in memory twice.
const CHUNK = 8 << 20;

let canvas;
let context;

async function opfs() {
  return navigator.storage.getDirectory();
}

// The picked disc into OPFS, under a temporary name first so a copy cut short is never taken for a disc.
async function importDisc(file) {
  const dir = await opfs();
  const part = await dir.getFileHandle(`${DISC}.part`, { create: true });
  const h = await part.createSyncAccessHandle();
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
  if (part.move) await part.move(dir, DISC);
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
  let handle;
  try {
    handle = await (await opfs()).getFileHandle(DISC);
  } catch {
    return null;
  }
  const h = await handle.createSyncAccessHandle();
  return { size: h.getSize(), read: (at, into) => h.read(into, { at }) };
}

async function start(file) {
  const disc = await openDisc(file);
  if (!disc) {
    postMessage({ missing: true });
    return;
  }
  let memory;
  const bytes = (ptr, len) => new Uint8Array(memory.buffer, ptr, len);
  const imports = {
    openmx: {
      disc_size: () => disc.size,
      disc_read: (at, ptr, len) => {
        try {
          return disc.read(at, bytes(ptr, len));
        } catch {
          return -1;
        }
      },
      present: (ptr, w, h) => {
        if (canvas.width !== w || canvas.height !== h) [canvas.width, canvas.height] = [w, h];
        context.putImageData(new ImageData(new Uint8ClampedArray(bytes(ptr, w * h * 4)), w, h), 0, 0);
      },
      log: (ptr, len) => console.log(new TextDecoder().decode(bytes(ptr, len))),
    },
  };
  const { instance } = await WebAssembly.instantiateStreaming(fetch('pkg/openmx_web.wasm'), imports);
  memory = instance.exports.memory;
  const err = instance.exports.openmx_start(canvas.width, canvas.height);
  if (err) {
    postMessage({ error: new TextDecoder().decode(bytes(instance.exports.openmx_message(), err)) });
    return;
  }
  instance.exports.openmx_frame();
  postMessage({ started: true });
}

onmessage = async ({ data }) => {
  try {
    if (data.start) {
      // The canvas comes once, with the first start; a later start (the picked file) draws in the same one.
      if (data.start.canvas) {
        canvas = data.start.canvas;
        context = canvas.getContext('2d');
      }
      await start(data.start.file);
    } else if (data.import) {
      await importDisc(data.import);
      await start();
    }
  } catch (e) {
    postMessage({ error: String(e) });
  }
};
