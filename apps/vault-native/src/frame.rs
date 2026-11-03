//! The browser's native messaging: a 32-bit length in the machine's own byte order, then that many bytes of JSON. Browsers refuse
//! a message from a host over 1 MiB. They would send one of up to 64 MiB or 4 GiB, and nothing the extension says comes near
//! 1 MiB, so that bounds this direction too, before anything is allocated.

use std::io::{self, ErrorKind, Read, Write};

use zeroize::Zeroizing;

pub const MAX_MESSAGE: usize = 1024 * 1024;

pub enum Frame {
    Message(Zeroizing<Vec<u8>>),
    /// The browser closed: it does that between messages, so this is no error
    End,
    /// What it said is too long or stopped halfway. There is no finding the next frame after either, so the host stops.
    Broken,
}

pub fn read(input: &mut impl Read) -> io::Result<Frame> {
    let mut prefix = [0u8; 4];
    let mut have = 0;
    while have < prefix.len() {
        match input.read(&mut prefix[have..]) {
            Ok(0) => return Ok(if have == 0 { Frame::End } else { Frame::Broken }),
            Ok(count) => have += count,
            Err(error) if error.kind() == ErrorKind::Interrupted => {}
            Err(error) => return Err(error),
        }
    }
    let length = u32::from_ne_bytes(prefix) as usize;
    if length > MAX_MESSAGE {
        return Ok(Frame::Broken);
    }
    let mut body = Zeroizing::new(vec![0u8; length]);
    match input.read_exact(&mut body) {
        Ok(()) => Ok(Frame::Message(body)),
        Err(error) if error.kind() == ErrorKind::UnexpectedEof => Ok(Frame::Broken),
        Err(error) => Err(error),
    }
}

/// One message, or `InvalidInput` for one the browser would refuse, with nothing written
pub fn write(output: &mut impl Write, message: &[u8]) -> io::Result<()> {
    if message.len() > MAX_MESSAGE {
        return Err(ErrorKind::InvalidInput.into());
    }
    output.write_all(&(message.len() as u32).to_ne_bytes())?;
    output.write_all(message)?;
    output.flush()
}
