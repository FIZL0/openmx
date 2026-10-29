---
license: other
license_name: per-mod
license_link: https://huggingface.co/datasets/FIZL0/openmx-mods#rules
pretty_name: openMX mod store
tags:
  - openmx
  - game-mods
size_categories:
  - n<1K
---

# openMX mod store

The mod store of [openMX](https://openmx.dev), an open remake of a motocross game's engine: tracks,
worlds, trick packs and music packs made by players, for players. openMX lists this dataset on its
**Mods → Browse the Store** screen (on the desktop and in the browser) and installs a mod with one press.
Players put their own mods here from **Mods → Upload to the Store**.

Every mod is the uploader's own work, shared under the terms its `mod.json`'s `permission` names.
Nothing here comes from a game disc: openMX reads the game's own files from the player's own copy, and
mods made from a disc stay on the player's machine.

## Rules

Uploading means agreeing to these, which openMX shows before the first upload:

1. **Uploads are public.** Anyone can download what goes up here.
2. **You are responsible for what you share.** Only upload mods you made, or that their makers let you
   give away. Name that permission in `mod.json`'s `permission` (`public-domain`, or a sentence naming
   who allowed it). openMX refuses a mod with no permission, and one marked `own-disc`.
3. **No game data.** Nothing copied or converted from a game disc or a game's install: no levels, models,
   textures, sounds or music of a published game. A mod built from the game's files belongs on the
   player's own machine, not here.
4. **Only files a mod needs.** A `mod.zip` holds data only: no programs, scripts or links. openMX checks
   every upload the same way it checks an install (below), and refuses anything else.
5. **Uploads are pull requests.** Each player's uploads go onto one open pull request under their
   Hugging Face account. openMX lists open pull requests at once, marked *not reviewed*; a mod becomes
   *Verified* when a maintainer merges it into `main`.
6. **Takedowns.** An uploader takes their own mod down from **My Mods** (it leaves their pull request,
   and an empty pull request is closed). Anyone can report a mod (openMX's **Report**, or a new
   discussion on the [Community](https://huggingface.co/datasets/FIZL0/openmx-mods/discussions) tab);
   maintainers remove what breaks these rules, and honour rights holders' takedown requests there.

## Layout

```
README.md            this card
index.json           the listing openMX reads first (optional; written by the maintainers)
mods/<id>/mod.zip    the mod: its folder, zipped, mod.json at the top
mods/<id>/meta.json  what the store lists without opening the zip
mods/<id>/preview.webp  a track's or world's picture (optional, at most 4 MB)
```

`<id>` is the mod's id from its `mod.json`: lowercase letters, digits and `_`. On `main` an id is one
mod; open pull requests may each carry their own copy of an id, told apart by their author.

`meta.json`:

```json
{
  "id": "my_track",
  "kind": "track",
  "name": "My Track",
  "author": "hf-user-name",
  "source": "Made in Blender",
  "version": "1.0.0",
  "size": 1234567,
  "sha256": "<mod.zip's sha256, lowercase hex>",
  "standard": 1
}
```

`kind` is `track`, `world`, `tricks` or `music`. `author` is the uploader's Hugging Face name, and
`standard` the version of the openmx-mods mod standard its `mod.json` follows.

`index.json` (when there is one) saves openMX from walking every pull request:

```json
{ "version": 1, "pulls": { "<n>": { "commit": "<the pull request's head commit>" } }, "entries": [ { "meta": { … }, "rev": "main", "verified": true } ] }
```

A pull request whose head moved since the index was written is read afresh.

## What openMX checks

Before an upload goes up and again before an install, openMX checks that a `mod.zip`:

- is a plain zip (stored or deflated, not encrypted), at most 4 GB, with at most 65,000 entries;
- has no absolute paths, `..`, links or duplicate names, and only data files: `json`, `txt`, `png`,
  `webp`, `ogg`, `wav`, `blend` and the game engine's own data formats;
- unpacks to at most 1 GB a file and 3.75 GB in all, measured while inflating;
- matches `meta.json`'s size and sha256, and has a `mod.json` whose `id` and `kind` are the store's;
- parses: a track's or world's files, a trick pack's and a music pack's, read by openMX's own readers.

An install goes into `mods/<kind>/<id>/` with a `store.json` beside its `mod.json`, and never over a
mod the player made themselves.

## Making a mod

The mod standard, the tools that build tracks, worlds, trick packs and music packs, and their
documentation live in the openmx-mods repository.
