//! Move to this computer (R7-25): the person's own folders, from the old computer to the new one over the home network.
//!
//! The parts: `crypto` (a key agreed for one transfer, the six digits both screens show, AES-256-GCM frames), `plan` (what
//! may cross, in one place that both sides ask), `session` (the conversation: confirm the digits, offer, choose, stream),
//! `zone` (only a network named home) and `discover` (finding the other computer). Nothing here is stored: the key is
//! in memory until the move ends, and a file that crosses is written only under the new computer's own home.

pub mod crypto;
pub mod discover;
pub mod plan;
pub mod session;
pub mod text;
pub mod zone;

/// The port the new computer listens on, TCP for the move and UDP for being found
pub const PORT: u16 = 4793;
