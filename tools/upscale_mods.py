#!/usr/bin/env python3
"""Upscale every mod's textures with chaiNNer, skipping the ones already done.

    python3 tools/upscale_mods.py MODEL.pth [MODS_DIR ...] [--chain tools/chainner/upscale.chn]
    python3 tools/upscale_mods.py --check [MODS_DIR ...]   # list what's missing or stale, per mod; exit 1 if any
    python3 tools/upscale_mods.py --prune [MODS_DIR ...]   # delete upscales no texture of the mod names any more

For each `mod.json` under MODS_DIR (default `mods/`), every texture the mod carries (its `.pst`s, `.atm`s
and its `.trn`s' ground pages, decoded by the port itself: `openmx --dump-textures OUT --from MOD`) without a
`textures/upscaled/<path in the mod>-<key>.png` yet is upscaled with the chain (its folders and model filled
in here, run headless with `chainner run`) and written there. The game draws those when the Upscaled
Textures setting is on. Mods with `"upscale": false` in mod.json are skipped, and so are texture packs
(`"kind": "textures"`), which are upscales already. Already-upscaled textures are skipped unless their source
changed (`textures/upscaled/sources.json`); delete a PNG to redo it. Rerun after adding or rebuilding mods.

The disc's own textures: `openmx --dump-textures` writes them to `mods/textures-src/`; upscale that folder
with the same chain in chaiNNer's window (or make it a texture pack, `mods/<name>/mod.json` with
`"kind": "textures"` and the PNGs under its `textures/`).

Slow (~2.5 s an image whatever its size)? chaiNNer 0.25.1 starts each node's work from a worker thread without
waking its event loop. In `resources/src/api/lazy.py` `Lazy.from_coroutine`, replace the `create_task` + polling
loop with `return asyncio.run_coroutine_threadsafe(coroutine, loop).result()` (20x faster on a GPU).
"""
import argparse, glob, hashlib, json, os, shutil, signal, subprocess, sys, tempfile
from pathlib import Path

REPO = Path(__file__).resolve().parents[1]

# each mod's textures, dumped once a run (`mod_textures`)
DUMPS = {}
DUMP_ROOT = None


def mod_textures(d, openmx):
    """(path under textures/upscaled, PNG bytes) of each texture mod folder `d` carries, as the port decodes
    and names it (`openmx --dump-textures --from`: `<path in the mod>-<key>.png`)."""
    global DUMP_ROOT
    if d not in DUMPS:
        if DUMP_ROOT is None:
            DUMP_ROOT = tempfile.mkdtemp(prefix='upscale-mods-')
        out = Path(DUMP_ROOT, str(len(DUMPS)))
        r = subprocess.run([openmx, '--dump-textures', str(out), '--from', str(d)], capture_output=True, text=True)
        if r.returncode:
            sys.exit(f'{d}: openmx --dump-textures failed ({r.returncode}): {r.stderr.strip() or r.stdout.strip()}')
        DUMPS[d] = [(f.relative_to(out).as_posix(), f.read_bytes()) for f in sorted(out.rglob('*.png'))] if out.exists() else []
    return DUMPS[d]


def sources(d):
    """`textures/upscaled/sources.json` of mod folder `d`: upscaled path -> SHA-1 of the image it was made from."""
    f = d / 'textures/upscaled/sources.json'
    return json.loads(f.read_text()) if f.exists() else {}


def mods(roots):
    """Every mod folder that wants upscales."""
    seen = set()
    for root in roots:
        # glob, not rglob: mods/ is often a symlink to a build folder
        for mj in sorted(glob.glob(f'{root}/**/mod.json', recursive=True)):
            d = Path(mj).parent
            # each mod once, however many symlinked paths reach it (a link back up loops ~40 deep)
            if d.resolve() in seen:
                continue
            seen.add(d.resolve())
            m = json.loads(Path(mj).read_text())
            if m.get('upscale', True) is False or m.get('kind') == 'textures':
                continue
            yield d


def todo(roots, openmx):
    """(mod folder, source PNG bytes, upscaled path) for every texture not upscaled yet or changed since."""
    for d in mods(roots):
        done = sources(d)
        for rel, png in mod_textures(d, openmx):
            out = d / 'textures/upscaled' / rel
            if not out.exists() or done.get(rel) != hashlib.sha1(png).hexdigest():
                yield d, png, out


def unused(roots, openmx):
    """(mod folder, upscaled path) of every upscale no texture of the mod names any more (a rebuilt or dropped
    one's): the game never draws it, but it still ships in the mod's zip."""
    for d in mods(roots):
        want = {rel for rel, _ in mod_textures(d, openmx)}
        up = d / 'textures/upscaled'
        for f in sorted(up.rglob('*.png')):
            if f.relative_to(up).as_posix() not in want:
                yield d, f


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument('model', type=Path, nargs='?', help='the 4x upscaling model (.pth etc.)')
    ap.add_argument('roots', nargs='*', default=[REPO / 'mods'], help='folders searched for mod.json')
    ap.add_argument('--chain', type=Path, default=REPO / 'tools/chainner/upscale.chn')
    ap.add_argument('--chainner', default=str(Path.home() / '.local/bin/chainner-app/chainner'), help='the chaiNNer binary')
    ap.add_argument('--openmx', default=str(REPO / 'target/release/openmx'), help='the port, which decodes the textures (cargo build --release -p openmx)')
    ap.add_argument('--batch', type=int, default=500, help='images per chaiNNer run')
    ap.add_argument('--check', nargs='*', metavar='MODS_DIR', help="only list what's missing or stale (default mods/)")
    ap.add_argument('--prune', nargs='*', metavar='MODS_DIR', help='delete unused upscales (default mods/)')
    a = ap.parse_args()
    try:
        if a.check is not None:
            return check(a.check or [REPO / 'mods'], a.openmx)
        if a.prune is not None:
            return prune(a.prune or [REPO / 'mods'], a.openmx)
        if a.model is None:
            ap.error('the model is required unless --check')
        run(a)
    finally:
        if DUMP_ROOT:
            shutil.rmtree(DUMP_ROOT, ignore_errors=True)


def run(a):
    jobs = list(todo(a.roots, a.openmx))
    if not jobs:
        print('nothing to upscale')
        return
    print(f'{len(jobs)} textures to upscale')
    done = 0
    # batches, so an interrupted run keeps what it finished
    for k in range(0, len(jobs), a.batch):
        done += upscale(jobs[k:k + a.batch], a)
        print(f'{done}/{len(jobs)} upscaled')
    if done < len(jobs):
        sys.exit('some textures were not written; rerun to retry them')


def check(roots, openmx):
    """Print each mod's missing or out-of-date upscales; exit 1 if there are any."""
    per = {}
    for d, _, out in todo(roots, openmx):
        per.setdefault(d, []).append(out)
    for d, outs in per.items():
        print(f'{d.name}: {len(outs)}')
        for o in outs:
            print(f"  {o.relative_to(d)}{'' if o.exists() else ' (missing)'}")
    print(f'{sum(map(len, per.values()))} textures in {len(per)} mods to upscale')
    left = {}
    for d, f in unused(roots, openmx):
        left[d.name] = left.get(d.name, 0) + 1
    if left:
        print(f"{sum(left.values())} unused upscales in {len(left)} mods (--prune deletes them): "
              + ', '.join(f'{k} {v}' for k, v in left.items()))
    if per:
        sys.exit(1)


def prune(roots, openmx):
    """Delete unused upscales and their `sources.json` entries."""
    gone = {}
    for d, f in list(unused(roots, openmx)):
        f.unlink()
        gone.setdefault(d, []).append(f.relative_to(d / 'textures/upscaled').as_posix())
    for d, keys in gone.items():
        m = {k: v for k, v in sources(d).items() if k not in keys}
        (d / 'textures/upscaled/sources.json').write_text(json.dumps(m, indent=0, sort_keys=True))
        # deepest first, so a folder emptied of folders goes too
        for sub in sorted((d / 'textures/upscaled').rglob('*'), key=lambda p: len(p.parts), reverse=True):
            if sub.is_dir() and not any(sub.iterdir()):
                sub.rmdir()
    print(f'{sum(map(len, gone.values()))} unused upscales deleted in {len(gone)} mods')


def upscale(jobs, a):
    """Run the chain on `jobs`, move the results into place; how many were written."""
    with tempfile.TemporaryDirectory() as tmp:
        src, dst = Path(tmp, 'in'), Path(tmp, 'out')
        src.mkdir(), dst.mkdir()
        # ponytail: flat numbered names, since the chain saves by file name only (no subfolders)
        for i, (_, png, _) in enumerate(jobs):
            (src / f'{i:06}.png').write_bytes(png)
        chain = json.loads(a.chain.read_text())
        # headless: drop preview nodes, which encode every result for a GUI that isn't there (most of the run time)
        c = chain['content']
        views = {n['id'] for n in c['nodes'] if n['data']['schemaId'] == 'chainner:image:view'}
        c['nodes'] = [n for n in c['nodes'] if n['id'] not in views]
        c['edges'] = [e for e in c['edges'] if e['target'] not in views]
        for n in c['nodes']:
            kind, data = n['data']['schemaId'], n['data']['inputData']
            if kind == 'chainner:image:load_images':
                data['0'] = str(src)
            elif kind == 'chainner:image:save':
                data['1'] = str(dst)
            elif kind == 'chainner:pytorch:load_model':
                data['0'] = str(a.model.resolve())
        run = Path(tmp, 'run.chn')
        run.write_text(json.dumps(chain))
        # --no-sandbox last: chaiNNer takes unknown options as positionals, so one before `run` eats the file
        # AMD (ROCm): MIOpen's default kernel search costs 15-20 s per new image size, in every chaiNNer run
        env = {'MIOPEN_FIND_MODE': 'FAST', **os.environ}
        p = subprocess.Popen([a.chainner, 'run', str(run), '--no-sandbox'], start_new_session=True, env=env)
        try:
            if p.wait():
                sys.exit(f'chaiNNer failed ({p.returncode})')
        finally:
            # its Python backend outlives `chainner run`: end the whole group
            try:
                os.killpg(p.pid, signal.SIGTERM)
            except ProcessLookupError:
                pass
        done, made = 0, {}
        for i, (d, png, out) in enumerate(jobs):
            f = dst / f'{i:06}.png'
            if f.exists():
                out.parent.mkdir(parents=True, exist_ok=True)
                shutil.move(f, out)
                made.setdefault(d, sources(d))[out.relative_to(d / 'textures/upscaled').as_posix()] = hashlib.sha1(png).hexdigest()
                done += 1
        for d, m in made.items():
            (d / 'textures/upscaled/sources.json').write_text(json.dumps(m, indent=0, sort_keys=True))
        return done

if __name__ == '__main__':
    main()
