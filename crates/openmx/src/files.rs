//! The shell's own files — its settings, controls, picks, presets, markers and online lobby — on both
//! builds (P196): `std::fs` on the desktop; in the browser, whose module has no file system, the
//! page's origin-private storage, where they sit beside the disc (`/opfs/`).
//!
//! The page reads every one of them before the shell starts and writes each back as it changes, so a
//! read and a write here are the desktop's: done before the call returns. What else the desktop
//! reads off its disk — mods, texture packs, music folders, recorded tapes — the page has none of.

#[cfg(not(target_arch = "wasm32"))]
pub use std::fs::{create_dir_all, read, read_to_string, remove_file, rename, write};

#[cfg(target_arch = "wasm32")]
pub use page::*;

#[cfg(target_arch = "wasm32")]
mod page {
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
}
