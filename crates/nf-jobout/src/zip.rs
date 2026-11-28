//! Stored (uncompressed) ZIP writer. Deterministic: fixed timestamp, fixed attributes, caller order.

use std::fmt;

const CRC_TABLE: [u32; 256] = {
    let mut t = [0u32; 256];
    let mut i = 0;
    while i < 256 {
        let mut c = i as u32;
        let mut k = 0;
        while k < 8 {
            c = if c & 1 != 0 {
                0xEDB8_8320 ^ (c >> 1)
            } else {
                c >> 1
            };
            k += 1;
        }
        t[i] = c;
        i += 1;
    }
    t
};

/// CRC-32 (IEEE), as used by ZIP.
pub fn crc32(data: &[u8]) -> u32 {
    let mut c = 0xFFFF_FFFFu32;
    for &b in data {
        c = CRC_TABLE[((c ^ b as u32) & 0xFF) as usize] ^ (c >> 8);
    }
    !c
}

#[derive(Debug, PartialEq, Eq)]
pub enum ZipError {
    DuplicateName(String),
    BadName(String),
    TooLarge,
}

impl fmt::Display for ZipError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ZipError::DuplicateName(n) => write!(f, "duplicate zip entry name: {n}"),
            ZipError::BadName(n) => write!(f, "bad zip entry name: {n}"),
            ZipError::TooLarge => write!(f, "zip too large for the non-zip64 format"),
        }
    }
}

impl std::error::Error for ZipError {}

struct Entry {
    name: String,
    crc: u32,
    size: u32,
    offset: u32,
}

/// Builds a stored ZIP in memory.
#[derive(Default)]
pub struct ZipWriter {
    buf: Vec<u8>,
    entries: Vec<Entry>,
}

// 1980-01-01 00:00:00 in DOS format.
const DOS_TIME: u16 = 0;
const DOS_DATE: u16 = 0x0021;

impl ZipWriter {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a file. Names are UTF-8, `/` separated, relative; the UTF-8 flag is set when non-ASCII.
    pub fn add(&mut self, name: &str, data: &[u8]) -> Result<(), ZipError> {
        if name.is_empty()
            || name.starts_with('/')
            || name.contains('\\')
            || name.contains('\0')
            || name
                .split('/')
                .any(|p| p.is_empty() || p == "." || p == "..")
            || name.len() > u16::MAX as usize
        {
            return Err(ZipError::BadName(name.to_string()));
        }
        if self.entries.iter().any(|e| e.name == name) {
            return Err(ZipError::DuplicateName(name.to_string()));
        }
        if self.entries.len() >= u16::MAX as usize - 1 {
            return Err(ZipError::TooLarge);
        }
        let size = u32::try_from(data.len()).map_err(|_| ZipError::TooLarge)?;
        let offset = u32::try_from(self.buf.len()).map_err(|_| ZipError::TooLarge)?;
        let crc = crc32(data);
        let flags: u16 = if name.is_ascii() { 0 } else { 0x0800 };
        let b = &mut self.buf;
        b.extend_from_slice(&0x0403_4b50u32.to_le_bytes());
        b.extend_from_slice(&20u16.to_le_bytes());
        b.extend_from_slice(&flags.to_le_bytes());
        b.extend_from_slice(&0u16.to_le_bytes()); // stored
        b.extend_from_slice(&DOS_TIME.to_le_bytes());
        b.extend_from_slice(&DOS_DATE.to_le_bytes());
        b.extend_from_slice(&crc.to_le_bytes());
        b.extend_from_slice(&size.to_le_bytes());
        b.extend_from_slice(&size.to_le_bytes());
        b.extend_from_slice(&(name.len() as u16).to_le_bytes());
        b.extend_from_slice(&0u16.to_le_bytes());
        b.extend_from_slice(name.as_bytes());
        b.extend_from_slice(data);
        if u32::try_from(self.buf.len()).is_err() {
            return Err(ZipError::TooLarge);
        }
        self.entries.push(Entry {
            name: name.to_string(),
            crc,
            size,
            offset,
        });
        Ok(())
    }

    /// Write the central directory and return the archive bytes.
    pub fn finish(mut self) -> Result<Vec<u8>, ZipError> {
        let cd_start = u32::try_from(self.buf.len()).map_err(|_| ZipError::TooLarge)?;
        let mut cd = Vec::new();
        for e in &self.entries {
            let flags: u16 = if e.name.is_ascii() { 0 } else { 0x0800 };
            cd.extend_from_slice(&0x0201_4b50u32.to_le_bytes());
            cd.extend_from_slice(&0x0314u16.to_le_bytes()); // made by: unix, 2.0
            cd.extend_from_slice(&20u16.to_le_bytes());
            cd.extend_from_slice(&flags.to_le_bytes());
            cd.extend_from_slice(&0u16.to_le_bytes());
            cd.extend_from_slice(&DOS_TIME.to_le_bytes());
            cd.extend_from_slice(&DOS_DATE.to_le_bytes());
            cd.extend_from_slice(&e.crc.to_le_bytes());
            cd.extend_from_slice(&e.size.to_le_bytes());
            cd.extend_from_slice(&e.size.to_le_bytes());
            cd.extend_from_slice(&(e.name.len() as u16).to_le_bytes());
            cd.extend_from_slice(&0u16.to_le_bytes());
            cd.extend_from_slice(&0u16.to_le_bytes());
            cd.extend_from_slice(&0u16.to_le_bytes());
            cd.extend_from_slice(&0u16.to_le_bytes());
            cd.extend_from_slice(&(0o100644u32 << 16).to_le_bytes());
            cd.extend_from_slice(&e.offset.to_le_bytes());
            cd.extend_from_slice(e.name.as_bytes());
        }
        let cd_len = u32::try_from(cd.len()).map_err(|_| ZipError::TooLarge)?;
        self.buf.extend_from_slice(&cd);
        let n = self.entries.len() as u16;
        let b = &mut self.buf;
        b.extend_from_slice(&0x0605_4b50u32.to_le_bytes());
        b.extend_from_slice(&0u16.to_le_bytes());
        b.extend_from_slice(&0u16.to_le_bytes());
        b.extend_from_slice(&n.to_le_bytes());
        b.extend_from_slice(&n.to_le_bytes());
        b.extend_from_slice(&cd_len.to_le_bytes());
        b.extend_from_slice(&cd_start.to_le_bytes());
        b.extend_from_slice(&0u16.to_le_bytes());
        if u32::try_from(self.buf.len()).is_err() {
            return Err(ZipError::TooLarge);
        }
        Ok(self.buf)
    }
}
