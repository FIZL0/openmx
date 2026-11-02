//! The shell's own files — its settings, controls, picks, presets, markers and online lobby — on both
//! builds (P196): `std::fs` on the desktop; in the browser, whose module has no file system, the
//! page's origin-private storage, where they sit beside the disc (`/opfs/`).
//!
//! The page reads every one of them before the shell starts and writes each back as it changes, so a
//! read and a write here are the desktop's: done before the call returns. What else the desktop
//! reads off its disk — texture packs, music folders, recorded tapes — the page has none of.
//!
//! The mods the store installed (P217) are the page's OPFS `mods/`, which the page lists before the
//! shell starts and keeps in step as the store puts and drops a mod's files; a file of it is read
//! when it is asked for. [`list`], [`is_dir`], [`is_file`] and [`folder`] read a mods folder on both
//! builds, so the shell's mods readers (`track_mods`, `trick_packs`, `music_packs`) list and parse the
//! page's installed mods as they do the desktop's folder.

use std::path::PathBuf;

/// One thing in a folder, as [`list`] gives it.
pub struct Listed {
    pub path: PathBuf,
    /// Whether it is a folder itself.
    pub dir: bool,
}

#[cfg(not(target_arch = "wasm32"))]
pub use std::fs::{create_dir_all, read, read_to_string, remove_file, rename, write};

#[cfg(target_arch = "wasm32")]
pub use page::*;

/// What is in the folder `dir`, in no order; a folder not there is an error.
#[cfg(not(target_arch = "wasm32"))]
pub fn list(dir: impl AsRef<std::path::Path>) -> std::io::Result<Vec<Listed>> {
    Ok(std::fs::read_dir(dir)?.flatten().map(|e| e.path()).map(|path| Listed { dir: path.is_dir(), path }).collect())
}

#[cfg(not(target_arch = "wasm32"))]
pub fn is_dir(path: impl AsRef<std::path::Path>) -> bool {
    path.as_ref().is_dir()
}

#[cfg(not(target_arch = "wasm32"))]
pub fn is_file(path: impl AsRef<std::path::Path>) -> bool {
    path.as_ref().is_file()
}

/// A mod's folder as an archive ([`game_render::disc::Res::folder`]): every file under it a member.
#[cfg(not(target_arch = "wasm32"))]
pub fn folder(root: &std::path::Path) -> Result<game_render::disc::Res, String> {
    game_render::disc::Res::folder(root)
}

#[cfg(target_arch = "wasm32")]
mod page {
    use super::Listed;
    use std::io::{Error, ErrorKind, Result};
    use std::path::Path;

    fn missing(path: &Path) -> Error {
        Error::new(ErrorKind::NotFound, format!("no {} in the page's storage", path.display()))
    }

    pub fn read(path: impl AsRef<Path>) -> Result<Vec<u8>> {
        crate::web_host::read(path.as_ref()).ok_or_else(|| missing(path.as_ref()))
    }

    pub fn read_to_string(path: impl AsRef<Path>) -> Result<String> {
        String::from_utf8(read(path)?).map_err(|e| Error::new(ErrorKind::InvalidData, e))
    }

    pub fn write(path: impl AsRef<Path>, bytes: impl AsRef<[u8]>) -> Result<()> {
        crate::web_host::write(path.as_ref(), Some(bytes.as_ref().to_vec()));
        Ok(())
    }

    pub fn rename(from: impl AsRef<Path>, to: impl AsRef<Path>) -> Result<()> {
        let bytes = read(&from)?;
        write(to, bytes)?;
        remove_file(from)
    }

    pub fn remove_file(path: impl AsRef<Path>) -> Result<()> {
        read(&path)?;
        crate::web_host::write(path.as_ref(), None);
        Ok(())
    }

    /// Folders are only the names' own: there is nothing to make.
    pub fn create_dir_all(_: impl AsRef<Path>) -> Result<()> {
        Ok(())
    }

    /// Every file under the folder `dir`, at any depth: `(path under it, /-separated; size)`.
    fn tree(dir: &Path) -> Vec<(String, u32)> {
        crate::web_host::tree(dir)
            .lines()
            .filter_map(|l| {
                let (path, size) = l.rsplit_once('\t')?;
                Some((path.to_string(), size.parse().ok()?))
            })
            .collect()
    }

    pub fn list(dir: impl AsRef<Path>) -> Result<Vec<Listed>> {
        let dir = dir.as_ref();
        let mut out: Vec<Listed> = Vec::new();
        for (path, _) in tree(dir) {
            let (name, deeper) = match path.split_once('/') {
                Some((name, _)) => (name, true),
                None => (path.as_str(), false),
            };
            if !out.iter().any(|l| l.path.file_name().is_some_and(|n| n == name)) {
                out.push(Listed { path: dir.join(name), dir: deeper });
            }
        }
        if out.is_empty() {
            return Err(missing(dir));
        }
        Ok(out)
    }

    /// A folder is the files under it: one with none is not there.
    pub fn is_dir(path: impl AsRef<Path>) -> bool {
        !tree(path.as_ref()).is_empty()
    }

    pub fn is_file(path: impl AsRef<Path>) -> bool {
        crate::web_host::size(path.as_ref()).is_some()
    }

    /// A mod's folder as an archive: listed now, each file read as it is asked for.
    pub fn folder(root: &Path) -> std::result::Result<game_render::disc::Res, String> {
        let at = root.to_path_buf();
        game_render::disc::Res::reader(tree(root), move |p| read(at.join(p)).map_err(|e| e.to_string()))
    }
}
