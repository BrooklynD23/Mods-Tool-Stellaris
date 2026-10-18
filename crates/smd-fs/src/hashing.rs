//! Streaming blake3 into a truncated [`Blob`].

use std::io::{self, Read};

use smd_core::Blob;

const BUF: usize = 64 * 1024;

/// Hash everything `r` yields, through a bounded buffer, into a 16-byte truncated blake3.
///
/// Reads at most `limit` bytes; the second element is `true` if more remained (the file
/// grew past what the caller measured).
///
/// # Errors
/// Any read error other than `Interrupted`.
pub fn hash_reader<R: Read>(mut r: R, limit: u64) -> io::Result<(Blob, bool)> {
    let mut h = blake3::Hasher::new();
    let mut buf = vec![0u8; BUF];
    let mut total: u64 = 0;
    loop {
        let want = usize::try_from((limit - total).min(BUF as u64)).unwrap_or(BUF);
        if want == 0 {
            let mut probe = [0u8; 1];
            let more = loop {
                match r.read(&mut probe) {
                    Ok(n) => break n > 0,
                    Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
                    Err(e) => return Err(e),
                }
            };
            return Ok((finish(&h, total), more));
        }
        match r.read(&mut buf[..want]) {
            Ok(0) => return Ok((finish(&h, total), false)),
            Ok(n) => {
                h.update(&buf[..n]);
                total += n as u64;
            }
            Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
            Err(e) => return Err(e),
        }
    }
}

fn finish(h: &blake3::Hasher, len: u64) -> Blob {
    let full = h.finalize();
    let mut hash = [0u8; 16];
    hash.copy_from_slice(&full.as_bytes()[..16]);
    Blob { hash, len }
}

#[cfg(test)]
mod tests;
