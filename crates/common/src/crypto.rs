use anyhow::{anyhow, Result};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use xsalsa20poly1305::{
    aead::{Aead, KeyInit, OsRng},
    XSalsa20Poly1305, 
};

use crate::CryptoHash;

pub fn blake3_hash(input: &[u8]) -> CryptoHash {
    let hash = blake3::hash(input);
    CryptoHash::new(hash.as_bytes().clone())
}

pub fn encrypt(text: &str, key: &str) -> Result<String> {
    // Create a key from the provided secret
    let key_bytes = blake3::hash(key.as_bytes());
    let cipher = XSalsa20Poly1305::new(key_bytes.as_bytes().into());

    // Generate a random 24-byte nonce
    let nonce = XSalsa20Poly1305::generate_nonce(&mut OsRng);

    // Encrypt the text
    let ciphertext = cipher
        .encrypt(&nonce, text.as_bytes())
        .map_err(|e| anyhow!("encryption failed: {}", e))?;

    // Combine nonce and ciphertext and encode as base64
    let mut combined = nonce.to_vec();
    combined.extend(ciphertext);
    Ok(URL_SAFE_NO_PAD.encode(combined))
}

pub fn decrypt(encrypted: &str, key: &str) -> Result<String> {
    // Create a key from the provided secret
    let key_bytes = blake3::hash(key.as_bytes());
    let cipher = XSalsa20Poly1305::new(key_bytes.as_bytes().into());

    // Decode the base64 input
    let encrypted_bytes = URL_SAFE_NO_PAD
        .decode(encrypted)
        .map_err(|e| anyhow!("invalid base64: {}", e))?;

    // Split into nonce and ciphertext
    if encrypted_bytes.len() < 24 {
        return Err(anyhow!("invalid encrypted data"));
    }
    let (nonce, ciphertext) = encrypted_bytes.split_at(24);

    // Decrypt the text
    let plaintext = cipher
        .decrypt(nonce.into(), ciphertext)
        .map_err(|e| anyhow!("decryption failed: {}", e))?;

    // Convert back to string
    String::from_utf8(plaintext)
        .map_err(|e| anyhow!("invalid utf8: {}", e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_blake3_hash() {
        let input = "Hello, World!";
        let hash = blake3_hash(input.as_bytes());

        let base64_hash = URL_SAFE_NO_PAD.encode(hash.hash());  
        println!("hash: {}", base64_hash);
    }

    #[test]
    fn test_encryption_decryption() {
        let key = "super_secret_key";
        let original_text = "Hello, World!";

        let encrypted = encrypt(original_text, key).unwrap();
        let decrypted = decrypt(&encrypted, key).unwrap();

        assert_eq!(original_text, decrypted);
    }

    #[test]
    fn test_decryption_with_wrong_key() {
        let key1 = "key1";
        let key2 = "key2";
        let original_text = "Hello, World!";

        let encrypted = encrypt(original_text, key1).unwrap();
        assert!(decrypt(&encrypted, key2).is_err());
    }
}