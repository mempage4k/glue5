//! The wire protocol, over one TCP connection.
//!
//! Every message starts with the same 32-byte header (little endian):
//!
//! | bytes | field | |
//! |---|---|---|
//! | 0 | kind | [`HELLO`], [`READ`], … |
//! | 1 | prio | [`DEMAND`] … [`FILL`] |
//! | 2..4 | reserved | 0 |
//! | 4..8 | req | the client's number for a request |
//! | 8..16 | off | a file offset, or an offset in a blob |
//! | 16..24 | len, file | a length (or a status) and a file id |
//! | 24..28 | crc | [`DATA`]: the CRC-32 of its bytes (TCP's own checksum is weak, and a bad block would stay in the copy) |
//! | 28..32 | reserved | 0 |
//!
//! [`HELLO`], [`BLOB`] and [`DATA`] are followed by `len` bytes.
//!
//! The server sends [`HELLO`] (JSON: protocol, build, catalog hash, blob
//! sizes) as soon as a client connects. The client then asks for byte ranges
//! of catalog files ([`READ`]) and for whole named blobs ([`BLOB`]: the
//! catalog, the packs, the boot sets, and the optional `packages` manifest),
//! each with a priority. Older clients ignore unknown optional HELLO blobs. The server
//! answers with [`DATA`] chunks and an [`END`] per request, always sending
//! the most urgent request's next chunk first: a read the engine is blocked
//! on overtakes megabytes of prefetch after at most one chunk. [`PRIORITY`]
//! moves a pending request to another priority (the engine now waits for
//! what was prefetched). Requests are never taken back: each gets its
//! [`END`].

use std::io::{self, Read, Write};

pub const PROTOCOL: u64 = 1;
/// The port a server listens on unless told otherwise.
pub const PORT: u16 = 47100;

pub const HELLO: u8 = 1;
pub const READ: u8 = 10;
pub const PRIORITY: u8 = 11;
pub const BLOB: u8 = 13;
pub const DATA: u8 = 20;
pub const END: u8 = 21;

/// An engine thread waits for it.
pub const DEMAND: u8 = 0;
/// The engine has queued the read (its streamer reads one at a time).
pub const HINT: u8 = 1;
/// The engine announced it may read it; sequential read-ahead.
pub const AHEAD: u8 = 2;
/// The boot set: read by every start, fetched before the engine asks.
pub const BOOT: u8 = 3;
/// Everything else, when the whole game is wanted on this device.
pub const FILL: u8 = 4;
pub const LEVELS: usize = 5;

/// [`END`] statuses (in `len`).
pub const DONE: u32 = 0;
/// The file ended before the range did (it is shorter than the catalog says).
pub const SHORT: u32 = 1;
pub const UNKNOWN: u32 = 2;
pub const FAILED: u32 = 3;

/// Data per [`DATA`] message: small enough that a demand read waits little
/// behind the chunk on the wire, large enough to cost few system calls.
pub const CHUNK: u32 = 64 * 1024;
/// The largest range one [`READ`] may ask for.
pub const MAX_READ: u32 = 16 << 20;
/// The largest payload of a [`HELLO`] or [`BLOB`] message.
pub const MAX_MESSAGE: u32 = 1 << 20;

pub const HEADER: usize = 32;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Header {
    pub kind: u8,
    pub prio: u8,
    pub req: u32,
    pub off: u64,
    pub len: u32,
    pub file: u32,
    pub crc: u32,
}

impl Header {
    pub fn encode(&self) -> [u8; HEADER] {
        let mut b = [0u8; HEADER];
        b[0] = self.kind;
        b[1] = self.prio;
        b[4..8].copy_from_slice(&self.req.to_le_bytes());
        b[8..16].copy_from_slice(&self.off.to_le_bytes());
        b[16..20].copy_from_slice(&self.len.to_le_bytes());
        b[20..24].copy_from_slice(&self.file.to_le_bytes());
        b[24..28].copy_from_slice(&self.crc.to_le_bytes());
        b
    }

    pub fn decode(b: &[u8; HEADER]) -> Header {
        let u32_at = |i: usize| u32::from_le_bytes(b[i..i + 4].try_into().unwrap());
        Header {
            kind: b[0],
            prio: b[1],
            req: u32_at(4),
            off: u64::from_le_bytes(b[8..16].try_into().unwrap()),
            len: u32_at(16),
            file: u32_at(20),
            crc: u32_at(24),
        }
    }

    pub fn read(r: &mut impl Read) -> io::Result<Header> {
        let mut b = [0u8; HEADER];
        r.read_exact(&mut b)?;
        Ok(Header::decode(&b))
    }

    /// Writes the header and its payload in one call (one segment for small
    /// messages, with Nagle off).
    pub fn send(&self, w: &mut impl Write, payload: &[u8]) -> io::Result<()> {
        let mut msg = Vec::with_capacity(HEADER + payload.len());
        msg.extend_from_slice(&self.encode());
        msg.extend_from_slice(payload);
        w.write_all(&msg)
    }
}

/// The checksum of a [`DATA`] message's bytes.
pub fn crc(data: &[u8]) -> u32 {
    crc32fast::hash(data)
}

/// Reads a message payload of `len` bytes, refusing absurd sizes.
pub fn payload(r: &mut impl Read, len: u32, max: u32) -> io::Result<Vec<u8>> {
    if len > max {
        return Err(io::Error::new(io::ErrorKind::InvalidData, format!("message of {len} bytes")));
    }
    let mut v = vec![0u8; len as usize];
    r.read_exact(&mut v)?;
    Ok(v)
}

#[test]
fn header_round_trip() {
    let h = Header { kind: READ, prio: HINT, req: 7, off: 1 << 40, len: 4096, file: 3, crc: 0xdead_beef };
    assert_eq!(Header::decode(&h.encode()), h);
}
