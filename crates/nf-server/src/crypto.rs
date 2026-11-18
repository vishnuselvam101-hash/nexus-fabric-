//! Password hashing, random tokens and blob encryption.
//!
//! Key management in this build is INTERIM: one AES-256-GCM key in a local key
//! file created on first start. There is no KMS/HSM integration, no per-tenant
//! keys and no rotation. The SQLite database itself is NOT encrypted at rest.

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use argon2::Argon2;
use std::path::Path;

pub fn random_bytes<const N: usize>() -> [u8; N] {
    let mut b = [0u8; N];
    rand::fill(&mut b[..]);
    b
}

pub fn hash_password(password: &str, salt: &[u8]) -> Result<String, String> {
    let mut out = [0u8; 32];
    Argon2::default().hash_password_into(password.as_bytes(), salt, &mut out).map_err(|e| e.to_string())?;
    Ok(hex::encode(out))
}

pub fn constant_time_eq(a: &str, b: &str) -> bool {
    a.len() == b.len() && a.bytes().zip(b.bytes()).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

pub fn load_or_create_key(path: &Path) -> std::io::Result<[u8; 32]> {
    if let Ok(bytes) = std::fs::read(path) {
        if bytes.len() == 32 {
            let mut k = [0u8; 32];
            k.copy_from_slice(&bytes);
            return Ok(k);
        }
        return Err(std::io::Error::other("key file has wrong length"));
    }
    let k: [u8; 32] = random_bytes();
    std::fs::write(path, k)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    }
    Ok(k)
}

/// Encrypt: 12-byte random nonce followed by ciphertext+tag.
pub fn seal(key: &[u8; 32], plain: &[u8]) -> Result<Vec<u8>, String> {
    let cipher = Aes256Gcm::new(&Key::<Aes256Gcm>::from(*key));
    let n: [u8; 12] = random_bytes();
    let ct = cipher.encrypt(&Nonce::from(n), plain).map_err(|_| "encryption failed".to_string())?;
    let mut out = n.to_vec();
    out.extend(ct);
    Ok(out)
}

pub fn open(key: &[u8; 32], sealed: &[u8]) -> Result<Vec<u8>, String> {
    if sealed.len() < 12 {
        return Err("sealed blob too short".into());
    }
    let cipher = Aes256Gcm::new(&Key::<Aes256Gcm>::from(*key));
    let mut n = [0u8; 12];
    n.copy_from_slice(&sealed[..12]);
    cipher.decrypt(&Nonce::from(n), &sealed[12..]).map_err(|_| "decryption failed (wrong key or corrupted blob)".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seal_open_round_trip_and_tamper_detection() {
        let k: [u8; 32] = random_bytes();
        let s = seal(&k, b"confidential drawing").unwrap();
        assert_ne!(&s[12..], b"confidential drawing");
        assert_eq!(open(&k, &s).unwrap(), b"confidential drawing");
        let mut t = s.clone();
        let last = t.len() - 1;
        t[last] ^= 1;
        assert!(open(&k, &t).is_err());
        let other: [u8; 32] = random_bytes();
        assert!(open(&other, &s).is_err());
        assert_ne!(seal(&k, b"x").unwrap(), seal(&k, b"x").unwrap(), "nonce must differ per message");
    }

    #[test]
    fn password_hash_is_salted_and_stable() {
        let h1 = hash_password("correct horse battery", b"0123456789abcdef").unwrap();
        assert_eq!(h1, hash_password("correct horse battery", b"0123456789abcdef").unwrap());
        assert_ne!(h1, hash_password("correct horse battery", b"fedcba9876543210").unwrap());
        assert_ne!(h1, hash_password("wrong", b"0123456789abcdef").unwrap());
        assert!(constant_time_eq("abc", "abc") && !constant_time_eq("abc", "abd") && !constant_time_eq("abc", "ab"));
    }
}
