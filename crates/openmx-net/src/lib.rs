//! Online play's transport and lobby (N3), brought over from opentennis's `hst-base` (`net.rs`, `handshake.rs`,
//! `chat.rs`): iroh QUIC peers with hole punching and the host relaying, four-character room codes, the lobby list
//! on the DHT and the LAN, and the handshake that agrees the seed and refuses a peer whose ride would differ.
//!
//! The front end drives it through [`Net`] ([`Cmd`] in, [`Event`] out); the ride's own traffic (N2's rollback
//! inputs) goes through [`Cmd::Send`] / [`Cmd::Datagram`] once [`Event::Start`] has come.

pub mod board;
pub mod chat;
pub mod codes;
pub mod handshake;
pub mod net;
pub mod room;

pub use handshake::{Disc, Handshake, Names};
pub use net::{Board, Cmd, Event, Net, Online, Relay};
pub use room::{HOST, Listing, Pick, Room, SEATS};
