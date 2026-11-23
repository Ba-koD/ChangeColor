use aes_gcm::aead::{Aead, AeadCore, KeyInit, OsRng};
use aes_gcm::{Aes256Gcm, Nonce};

use crate::{Error, user_error};

/// Header written in front of an encrypted state file, followed by the nonce and ciphertext.
const MAGIC: &[u8] = b"CCENC1\n";
const NONCE_LEN: usize = 12;

/// Encrypts the state file at rest with AES-256-GCM.
pub(crate) struct StateCipher(Aes256Gcm);

impl StateCipher {
    /// Parses a 32-byte key written as 64 hex characters (e.g. from `openssl rand -hex 32`).
    pub(crate) fn from_hex(hex: &str) -> Result<Self, Error> {
        let hex = hex.trim();
        if hex.len() != 64 || !hex.is_ascii() {
            return Err(user_error(
                "DATA_ENCRYPTION_KEY must be 64 hex characters (32 bytes)",
            ));
        }
        let mut key = [0u8; 32];
        for (index, byte) in key.iter_mut().enumerate() {
            *byte = u8::from_str_radix(&hex[index * 2..index * 2 + 2], 16)
                .map_err(|_| user_error("DATA_ENCRYPTION_KEY must be hex"))?;
        }
        let cipher = Aes256Gcm::new_from_slice(&key)
            .map_err(|_| user_error("DATA_ENCRYPTION_KEY has an invalid length"))?;
        Ok(Self(cipher))
    }

    pub(crate) fn encrypt(&self, plaintext: &[u8]) -> Result<Vec<u8>, Error> {
        let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
        let ciphertext = self
            .0
            .encrypt(&nonce, plaintext)
            .map_err(|_| user_error("failed to encrypt state"))?;
        let mut out = Vec::with_capacity(MAGIC.len() + NONCE_LEN + ciphertext.len());
        out.extend_from_slice(MAGIC);
        out.extend_from_slice(&nonce);
        out.extend_from_slice(&ciphertext);
        Ok(out)
    }

    pub(crate) fn decrypt(&self, data: &[u8]) -> Result<Vec<u8>, Error> {
        let body = data
            .strip_prefix(MAGIC)
            .filter(|body| body.len() >= NONCE_LEN)
            .ok_or_else(|| user_error("state file is not a valid encrypted file"))?;
        let (nonce, ciphertext) = body.split_at(NONCE_LEN);
        self.0
            .decrypt(Nonce::from_slice(nonce), ciphertext)
            .map_err(|_| user_error("failed to decrypt state: wrong DATA_ENCRYPTION_KEY?"))
    }
}

pub(crate) fn is_encrypted(data: &[u8]) -> bool {
    data.starts_with(MAGIC)
}
