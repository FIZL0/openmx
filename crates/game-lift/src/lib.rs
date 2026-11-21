//! The S496 pilot: the original's code **lifted** rather than ported. Each function here is the
//! original's, translated close to line for line from its disassembly, and it runs over [`Mem`] — the
//! original's own memory, laid out as the original lays it out, so an object is a pointer and a field
//! is `this + offset` exactly as the original reads it. Nothing is re-modelled into Rust structs.
//!
//! That is what makes a lift cheap to prove: the oracle runs the original over a RAM image, the lift
//! runs over the same bytes, and the memory each leaves behind must match byte for byte
//! (`tests/lift.rs`). No field has to be mapped in or out of a port's own types.
//!
//! Floats go through `ps2-float`'s [`F`] and [`Vf`], never host `f32`, so a lift stays as deterministic
//! as the port on every platform. Offsets are the original's struct offsets; where a function lived in
//! the original goes in `research/oracle.txt`, never here.
#![forbid(unsafe_code)]
#![deny(clippy::float_arithmetic)] // the lint that keeps a bit-exact lift bit-exact

pub mod ai;
pub mod bridge;
pub mod gate;

use ps2_float::{F, Vf};

/// The EE's main memory, 32 MiB.
pub const RAM: usize = 32 << 20;

/// The original's memory: a flat byte array addressed by the original's own pointers. Reads and writes
/// are little-endian, as the EE's are; an address is taken modulo the RAM's size, the way the EE's
/// cached, uncached and mirror segments all reach the same bytes.
#[derive(Clone)]
pub struct Mem {
    bytes: Vec<u8>,
}

impl Default for Mem {
    fn default() -> Mem {
        Mem::new()
    }
}

impl Mem {
    /// 32 MiB of zeros.
    pub fn new() -> Mem {
        Mem { bytes: vec![0; RAM] }
    }

    /// `len` bytes of zeros: a memory for a bridge, which lays out only the few objects a lifted
    /// function reads.
    pub fn sized(len: usize) -> Mem {
        Mem { bytes: vec![0; len] }
    }

    /// A RAM image as the original left it (a save state's, or the oracle's after a call).
    pub fn from_ram(mut bytes: Vec<u8>) -> Mem {
        bytes.resize(RAM, 0);
        Mem { bytes }
    }

    /// The bytes, for a comparison or a hash.
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    fn at(&self, a: u32) -> usize {
        a as usize % self.bytes.len()
    }

    pub fn u8(&self, a: u32) -> u8 {
        self.bytes[self.at(a)]
    }
    pub fn set_u8(&mut self, a: u32, v: u8) {
        let i = self.at(a);
        self.bytes[i] = v;
    }
    pub fn u16(&self, a: u32) -> u16 {
        let i = self.at(a);
        u16::from_le_bytes([self.bytes[i], self.bytes[i + 1]])
    }
    pub fn set_u16(&mut self, a: u32, v: u16) {
        let i = self.at(a);
        self.bytes[i..i + 2].copy_from_slice(&v.to_le_bytes());
    }
    pub fn u32(&self, a: u32) -> u32 {
        let i = self.at(a);
        u32::from_le_bytes(self.bytes[i..i + 4].try_into().expect("four bytes"))
    }
    pub fn set_u32(&mut self, a: u32, v: u32) {
        let i = self.at(a);
        self.bytes[i..i + 4].copy_from_slice(&v.to_le_bytes());
    }
    pub fn i32(&self, a: u32) -> i32 {
        self.u32(a) as i32
    }
    pub fn set_i32(&mut self, a: u32, v: i32) {
        self.set_u32(a, v as u32);
    }

    /// A float word as `lwc1` loads it: the raw pattern, which the FPU then reads as an operand.
    pub fn f(&self, a: u32) -> F {
        F(f32::from_bits(self.u32(a)))
    }
    /// A float word as `swc1` stores it: the pattern, unchanged.
    pub fn set_f(&mut self, a: u32, v: F) {
        self.set_u32(a, v.to_bits());
    }

    /// A quadword as `lqc2` loads it into a VU0 register: four raw patterns. The EE ignores the low
    /// four bits of a quadword address.
    pub fn vf(&self, a: u32) -> Vf {
        let a = a & !15;
        Vf([0, 4, 8, 12].map(|o| f32::from_bits(self.u32(a + o))))
    }
    /// A quadword as `sqc2` stores it.
    pub fn set_vf(&mut self, a: u32, v: Vf) {
        let a = a & !15;
        for (k, w) in v.0.iter().enumerate() {
            self.set_u32(a + 4 * k as u32, w.to_bits());
        }
    }
}
