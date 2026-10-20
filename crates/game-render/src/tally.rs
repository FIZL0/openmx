//! How many times the slow things have happened in this process: reads off the disc image, members
//! inflated out of a `.res`, textures decoded (P47). A shell that loads everything before its first
//! frame should see these stand still after it; `openmx --timings` prints what moved, and a test
//! holds it to zero.

use std::sync::atomic::{AtomicUsize, Ordering};

static READS: AtomicUsize = AtomicUsize::new(0);
static INFLATES: AtomicUsize = AtomicUsize::new(0);
static DECODES: AtomicUsize = AtomicUsize::new(0);

/// The three counts so far, in one value to subtract a later one from.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Tally {
    /// Reads off the disc image (each a seek and a read).
    pub reads: usize,
    /// Members read out of a `.res` archive.
    pub inflates: usize,
    /// `.pst` textures decoded.
    pub decodes: usize,
}

impl Tally {
    pub fn now() -> Tally {
        Tally {
            reads: READS.load(Ordering::Relaxed),
            inflates: INFLATES.load(Ordering::Relaxed),
            decodes: DECODES.load(Ordering::Relaxed),
        }
    }

    /// What happened between `earlier` and this one.
    pub fn since(self, earlier: Tally) -> Tally {
        Tally {
            reads: self.reads - earlier.reads,
            inflates: self.inflates - earlier.inflates,
            decodes: self.decodes - earlier.decodes,
        }
    }
}

pub(crate) fn read() {
    READS.fetch_add(1, Ordering::Relaxed);
}

pub(crate) fn inflate() {
    INFLATES.fetch_add(1, Ordering::Relaxed);
}

pub(crate) fn decode() {
    DECODES.fetch_add(1, Ordering::Relaxed);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_decode_moves_the_tally() {
        let before = Tally::now();
        let _ = crate::pst::Texture::parse(&[]);
        assert!(Tally::now().since(before).decodes >= 1);
    }
}
