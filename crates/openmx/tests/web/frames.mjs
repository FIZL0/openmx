// The game worker's own frames, for the pad drivers (K39; gamepad.mjs, firefox_gamepad.mjs). The worker reads the pads
// once a frame of its own (web_host's PadNode), and a page slowed by load has far fewer of those a second: a Cross
// timed in the driver's milliseconds — 100 ms down, the next 1.5 s on — went down and up between two of its frames,
// and twelve of them ran out before the menus had seen three (`taps: 12, seated: false`, K27's merge gate). So a press
// is held for the worker's frames instead.
//
// WORKER goes ahead of worker.js as the driver serves it, as gpu.mjs's fault does: it counts the worker's animation
// frames and keeps the Cross (standard button 0) of the first pad in the page's last `{ pads }` message, and posts
// `[frames, cross]` on a BroadcastChannel each frame. PAGE, run in the page, keeps the last of those in
// `window.openmxTestSeen` ([0, 0] until the worker has drawn).

export const CHANNEL = 'openmx-test-frames';

export const WORKER = `{
  const told = new BroadcastChannel(${JSON.stringify(CHANNEL)});
  let frames = 0;
  let cross = 0;
  addEventListener('message', ({ data }) => {
    if (data?.pads) cross = data.pads[1] ?? 0;
  });
  const raf = self.requestAnimationFrame?.bind(self);
  const counted = f => t => {
    frames++;
    told.postMessage([frames, cross]);
    f(t);
  };
  self.requestAnimationFrame = raf ? f => raf(counted(f)) : f => setTimeout(() => counted(f)(performance.now()), 16);
}
`;

export const PAGE = `{
  window.openmxTestSeen = [0, 0];
  new BroadcastChannel(${JSON.stringify(CHANNEL)}).onmessage = ({ data }) => (window.openmxTestSeen = data);
}`;

// The driver's side: `seen()` reads the page's `openmxTestSeen`; `wait(ms)` sleeps. `held(v, n, ms)`: true once `n` of the
// worker's frames have gone by since it first heard Cross at `v`, false if not within `ms` (the bound on a failure only:
// a run that passes returns as soon as the frames are). `within(n, ok, ms)`: waits for `ok()` over the next `n` frames
// (and `ms` at most) and says whether it came.
export const frames = (seen, wait) => {
  const held = async (v, n, ms) => {
    const t = Date.now();
    let since = null;
    for (;;) {
      const [count, cross] = (await seen()) ?? [0, 0];
      if (since === null && cross === v) since = count;
      if (since !== null && count >= since + n) return true;
      if (Date.now() - t > ms) return false;
      await wait(20);
    }
  };
  const within = async (n, ok, ms) => {
    const t = Date.now();
    const [start] = (await seen()) ?? [0, 0];
    for (;;) {
      if (ok()) return true;
      if (((await seen()) ?? [0, 0])[0] >= start + n || Date.now() - t > ms) return ok();
      await wait(50);
    }
  };
  return { held, within };
};
