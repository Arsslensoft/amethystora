//! A key for one move, and the six digits that prove nobody is in between.
//!
//! The old computer (the sender) and the new one (the receiver) agree a key with P-256 and show the same six digits, which the
//! person compares. The digits are not a password: a program on the network that sits between the two must make the digits
//! on both screens equal without knowing either side's key yet, and the commitment below is what stops it. The sender commits
//! to its public key and a nonce first (a hash), the receiver answers with its own, and only then does the sender reveal; the
//! digits come from all four, so a stranger who has to choose its key before it has seen the other side's cannot grind for
//! a pair that shows alike (one in a million at best, once, and the person is looking).
//!
//! The keys live in `Keys` (wiped when it is dropped) and are never written, never put in an argument or an environment
//! variable and never printed. Each direction has a key of its own and a counter for a nonce, so that no nonce is used twice
//! under a key; a frame that was changed, dropped or repeated fails to open and the move stops.

use aes_gcm::aead::{Aead, KeyInit, Payload};
use aes_gcm::{Aes256Gcm, Nonce};
use p256::{NonZeroScalar, PublicKey};
use sha2::{Digest, Sha256};
use zeroize::{Zeroize, Zeroizing};

/// The longest a sealed frame may be: a megabyte of data, the frame's type byte and its tag
pub const MOST_FRAME: usize = 1 << 20;

#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    /// A key that is not a point on the curve, or the system could not make random bytes
    Key,
    /// The commitment did not match what was revealed
    Commitment,
    /// A frame did not open
    Frame,
}

/// Who started: the old computer sends, the new one receives
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Sender,
    Receiver,
}

/// One side's half of the exchange: a secret for this move alone, the public key and the nonce that go to the other side
pub struct Ephemeral {
    secret: Zeroizing<[u8; 32]>,
    pub public: [u8; 65],
    pub nonce: [u8; 16],
}

impl Ephemeral {
    pub fn new() -> Result<Ephemeral, Error> {
        loop {
            let mut bytes = Zeroizing::new([0u8; 32]);
            getrandom::fill(&mut bytes[..]).map_err(|_| Error::Key)?;
            // Almost always valid; a value outside the curve's order is simply drawn again
            let Some(scalar) = Option::<NonZeroScalar>::from(NonZeroScalar::from_repr((*bytes).into())) else { continue };
            let public = PublicKey::from_secret_scalar(&scalar).to_sec1_bytes();
            let mut nonce = [0u8; 16];
            getrandom::fill(&mut nonce).map_err(|_| Error::Key)?;
            return Ok(Ephemeral { secret: bytes, public: public.as_ref().try_into().map_err(|_| Error::Key)?, nonce });
        }
    }

    /// What the sender sends first: a hash of what it will reveal, which fixes its key and nonce
    pub fn commitment(&self) -> [u8; 32] {
        commit(&self.public, &self.nonce)
    }
}

pub fn commit(public: &[u8], nonce: &[u8]) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(b"amethystora-move commit");
    hash.update(public);
    hash.update(nonce);
    hash.finalize().into()
}

/// What is agreed: the digits to compare and a key for each direction
pub struct Keys {
    pub digits: String,
    send: Zeroizing<[u8; 32]>,
    receive: Zeroizing<[u8; 32]>,
    send_count: u64,
    receive_count: u64,
    /// Which direction this side's frames are, in the data the tag also covers
    send_tag: u8,
    receive_tag: u8,
}

/// Works out the digits and the keys. `sender` is the sender's half and `receiver` the receiver's, as each side has them
/// (the other's public key and nonce as they were sent, the own as they were made).
pub fn agree(role: Role, own: &Ephemeral, other_public: &[u8], other_nonce: &[u8; 16]) -> Result<Keys, Error> {
    let other = PublicKey::from_sec1_bytes(other_public).map_err(|_| Error::Key)?;
    let scalar = Option::<NonZeroScalar>::from(NonZeroScalar::from_repr((*own.secret).into())).ok_or(Error::Key)?;
    let shared = (other.to_projective() * *scalar).to_affine();
    let mut x = Zeroizing::new([0u8; 32]);
    x.copy_from_slice(&p256::elliptic_curve::point::AffineCoordinates::x(&shared));

    let (sender_public, sender_nonce, receiver_public, receiver_nonce): (&[u8], &[u8], &[u8], &[u8]) = match role {
        Role::Sender => (&own.public, &own.nonce, other_public, other_nonce),
        Role::Receiver => (other_public, other_nonce, &own.public, &own.nonce),
    };
    let mut transcript = Sha256::new();
    transcript.update(b"amethystora-move transcript");
    for part in [sender_public, sender_nonce, receiver_public, receiver_nonce] {
        transcript.update(part);
    }
    let transcript: [u8; 32] = transcript.finalize().into();

    let digest = |label: &[u8], secret: &[u8]| -> [u8; 32] {
        let mut hash = Sha256::new();
        hash.update(label);
        hash.update(secret);
        hash.update(transcript);
        hash.finalize().into()
    };
    // The digits come from the transcript alone, which both screens may show without giving the keys away
    let number = u32::from_be_bytes(digest(b"digits", b"")[..4].try_into().expect("four bytes")) % 1_000_000;
    let mut to_receiver = digest(b"to receiver", &x[..]);
    let mut to_sender = digest(b"to sender", &x[..]);
    let (send, receive, send_tag, receive_tag) = match role {
        Role::Sender => (Zeroizing::new(to_receiver), Zeroizing::new(to_sender), 1, 2),
        Role::Receiver => (Zeroizing::new(to_sender), Zeroizing::new(to_receiver), 2, 1),
    };
    to_receiver.zeroize();
    to_sender.zeroize();
    Ok(Keys { digits: format!("{number:06}"), send, receive, send_count: 0, receive_count: 0, send_tag, receive_tag })
}

fn nonce_of(count: u64) -> [u8; 12] {
    let mut nonce = [0u8; 12];
    nonce[4..].copy_from_slice(&count.to_be_bytes());
    nonce
}

impl Keys {
    /// Seals one frame: its type and its body. The counter is the nonce, so the order is part of what is checked.
    pub fn seal(&mut self, kind: u8, body: &[u8]) -> Vec<u8> {
        let cipher = Aes256Gcm::new_from_slice(&self.send[..]).expect("a 256-bit key");
        let nonce = Nonce::try_from(&nonce_of(self.send_count)[..]).expect("a 96-bit nonce");
        self.send_count += 1;
        let mut plain = Vec::with_capacity(1 + body.len());
        plain.push(kind);
        plain.extend_from_slice(body);
        let aad = [self.send_tag];
        let sealed = cipher.encrypt(&nonce, Payload { msg: &plain, aad: &aad }).expect("AES-GCM seals anything that fits in memory");
        plain.zeroize();
        sealed
    }

    /// Opens the next frame from the other side: its type and body, or an error for anything that is not exactly the next
    /// frame the other side sealed
    pub fn open(&mut self, sealed: &[u8]) -> Result<(u8, Vec<u8>), Error> {
        let cipher = Aes256Gcm::new_from_slice(&self.receive[..]).map_err(|_| Error::Frame)?;
        let nonce = Nonce::try_from(&nonce_of(self.receive_count)[..]).map_err(|_| Error::Frame)?;
        let aad = [self.receive_tag];
        let mut plain = cipher.decrypt(&nonce, Payload { msg: sealed, aad: &aad }).map_err(|_| Error::Frame)?;
        self.receive_count += 1;
        if plain.is_empty() {
            return Err(Error::Frame);
        }
        let kind = plain.remove(0);
        Ok((kind, plain))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pair() -> (Keys, Keys) {
        let sender = Ephemeral::new().unwrap();
        let receiver = Ephemeral::new().unwrap();
        let on_sender = agree(Role::Sender, &sender, &receiver.public, &receiver.nonce).unwrap();
        let on_receiver = agree(Role::Receiver, &receiver, &sender.public, &sender.nonce).unwrap();
        (on_sender, on_receiver)
    }

    #[test]
    fn both_sides_show_the_same_six_digits_and_talk_in_both_directions() {
        let (mut a, mut b) = pair();
        assert_eq!(a.digits, b.digits);
        assert_eq!(a.digits.len(), 6);
        assert!(a.digits.bytes().all(|c| c.is_ascii_digit()));
        let sealed = a.seal(7, b"hello");
        assert_eq!(b.open(&sealed).unwrap(), (7, b"hello".to_vec()));
        let back = b.seal(8, b"and back");
        assert_eq!(a.open(&back).unwrap(), (8, b"and back".to_vec()));
    }

    #[test]
    fn a_stranger_between_the_two_shows_other_digits_and_cannot_read() {
        let sender = Ephemeral::new().unwrap();
        let receiver = Ephemeral::new().unwrap();
        let stranger = Ephemeral::new().unwrap();
        // The sender thinks the stranger is the receiver, and the receiver thinks the stranger is the sender
        let mut on_sender = agree(Role::Sender, &sender, &stranger.public, &stranger.nonce).unwrap();
        let on_receiver = agree(Role::Receiver, &receiver, &stranger.public, &stranger.nonce).unwrap();
        // One in a million alike by chance; the two keys are never alike
        let sealed = on_sender.seal(1, b"secret");
        let mut as_stranger_of_receiver = agree(Role::Sender, &stranger, &receiver.public, &receiver.nonce).unwrap();
        assert!(as_stranger_of_receiver.open(&sealed).is_err(), "what the sender sealed does not open for the receiver's side");
        assert_ne!(on_sender.digits, on_receiver.digits, "the digits differ (a one-in-a-million chance aside)");
    }

    #[test]
    fn a_changed_dropped_repeated_or_reordered_frame_does_not_open() {
        let (mut a, mut b) = pair();
        let first = a.seal(1, b"one");
        let second = a.seal(1, b"two");
        let mut changed = first.clone();
        *changed.last_mut().unwrap() ^= 1;
        assert!(b.open(&changed).is_err());
        assert!(b.open(&second).is_err(), "the second frame is not the next one");
        assert!(b.open(&first).is_ok());
        assert!(b.open(&first).is_err(), "a frame cannot be played twice");
        assert!(b.open(&second).is_ok());
    }

    #[test]
    fn a_frame_sent_back_to_its_sender_does_not_open() {
        let (mut a, _b) = pair();
        let sealed = a.seal(1, b"mine");
        assert!(a.open(&sealed).is_err(), "each direction has a key of its own");
    }

    #[test]
    fn the_commitment_fixes_the_key_and_the_nonce() {
        let e = Ephemeral::new().unwrap();
        assert_eq!(e.commitment(), commit(&e.public, &e.nonce));
        let mut other = e.nonce;
        other[0] ^= 1;
        assert_ne!(e.commitment(), commit(&e.public, &other));
    }

    #[test]
    fn a_key_off_the_curve_is_refused() {
        let e = Ephemeral::new().unwrap();
        let mut off = e.public;
        off[40] ^= 0xff;
        assert!(agree(Role::Sender, &e, &off, &[0; 16]).is_err());
        assert!(agree(Role::Sender, &e, &[1, 2, 3], &[0; 16]).is_err());
    }
}
