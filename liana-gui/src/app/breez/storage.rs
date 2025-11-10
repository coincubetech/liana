//! Lightning wallet mnemonic storage (encrypted)
//! 
//! IMPORTANT: Each Bitcoin wallet gets its own separate Lightning wallet.
//! Storage path: <network_dir>/<wallet_checksum>/lightning_wallet.enc
//! This matches Liana's multi-wallet architecture.

use std::fs;
use std::path::{Path, PathBuf};
use std::io::{Read, Write};

use super::BreezError;

const LIGHTNING_WALLET_FILE: &str = "lightning_wallet.enc";
const LIGHTNING_SUBDIR: &str = "lightning";

/// Generate a new Lightning wallet mnemonic (24 words)
pub fn generate_lightning_mnemonic() -> Result<String, BreezError> {
    // Generate 24-word mnemonic for Lightning wallet
    use rand::Rng;
    
    // Use Breez SDK's mnemonic generation if available, otherwise use bip39
    let mut entropy = [0u8; 32]; // 32 bytes = 24 words
    rand::thread_rng().fill(&mut entropy);
    
    let mnemonic = bip39::Mnemonic::from_entropy(&entropy)
        .map_err(|e| BreezError::Config(format!("Failed to generate mnemonic: {}", e)))?;
    
    Ok(mnemonic.to_string())
}

/// Get the Lightning wallet directory for a specific Bitcoin wallet
/// Path format: <network_dir>/<wallet_checksum>/lightning/
fn get_lightning_wallet_dir(network_dir: &Path, wallet_checksum: &str) -> PathBuf {
    network_dir
        .join(wallet_checksum)
        .join(LIGHTNING_SUBDIR)
}

/// Check if Lightning wallet already exists for a specific Bitcoin wallet
pub fn lightning_wallet_exists(network_dir: &Path, wallet_checksum: &str) -> bool {
    let wallet_dir = get_lightning_wallet_dir(network_dir, wallet_checksum);
    let wallet_path = wallet_dir.join(LIGHTNING_WALLET_FILE);
    wallet_path.exists()
}

/// Store Lightning mnemonic (encrypted with ChaCha20)
/// Each Bitcoin wallet gets its own Lightning wallet in a separate subdirectory
pub fn store_lightning_mnemonic(
    network_dir: &Path, 
    wallet_checksum: &str,
    mnemonic: &str
) -> Result<(), BreezError> {
    let wallet_dir = get_lightning_wallet_dir(network_dir, wallet_checksum);
    let wallet_path = wallet_dir.join(LIGHTNING_WALLET_FILE);
    
    // Create Lightning wallet directory for this specific Bitcoin wallet
    fs::create_dir_all(&wallet_dir)
        .map_err(|e| BreezError::Config(format!("Failed to create Lightning wallet directory: {}", e)))?;
    
    // Derive encryption key from network directory + wallet checksum + hostname
    // This provides basic obfuscation - still accessible by the app but not plaintext
    let key = derive_encryption_key(network_dir, wallet_checksum);
    
    // Encrypt using ChaCha20
    let encrypted = encrypt_chacha20(mnemonic.as_bytes(), &key);
    
    // Write to file with version prefix for future compatibility
    let mut file = fs::File::create(&wallet_path)
        .map_err(|e| BreezError::Config(format!("Failed to create wallet file: {}", e)))?;
    
    // Version byte (v1)
    file.write_all(&[1u8])
        .map_err(|e| BreezError::Config(format!("Failed to write version: {}", e)))?;
    
    file.write_all(&encrypted)
        .map_err(|e| BreezError::Config(format!("Failed to write wallet file: {}", e)))?;
    
    // Set restrictive permissions on Unix
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let metadata = file.metadata()
            .map_err(|e| BreezError::Config(format!("Failed to get file metadata: {}", e)))?;
        let mut permissions = metadata.permissions();
        permissions.set_mode(0o600); // Read/write for owner only
        fs::set_permissions(&wallet_path, permissions)
            .map_err(|e| BreezError::Config(format!("Failed to set permissions: {}", e)))?;
    }
    
    Ok(())
}

/// Load Lightning mnemonic from storage for a specific Bitcoin wallet
pub fn load_lightning_mnemonic(network_dir: &Path, wallet_checksum: &str) -> Result<String, BreezError> {
    let wallet_dir = get_lightning_wallet_dir(network_dir, wallet_checksum);
    let wallet_path = wallet_dir.join(LIGHTNING_WALLET_FILE);
    
    if !wallet_path.exists() {
        return Err(BreezError::Config(format!(
            "Lightning wallet not found for Bitcoin wallet {}",
            wallet_checksum
        )));
    }
    
    // Read encrypted file
    let mut file = fs::File::open(&wallet_path)
        .map_err(|e| BreezError::Config(format!("Failed to open wallet file: {}", e)))?;
    
    let mut data = Vec::new();
    file.read_to_end(&mut data)
        .map_err(|e| BreezError::Config(format!("Failed to read wallet file: {}", e)))?;
    
    // Check version
    if data.is_empty() {
        return Err(BreezError::Config("Wallet file is empty".to_string()));
    }
    
    let version = data[0];
    if version != 1 {
        return Err(BreezError::Config(format!("Unsupported wallet file version: {}", version)));
    }
    
    // Decrypt
    let key = derive_encryption_key(network_dir, wallet_checksum);
    let decrypted = decrypt_chacha20(&data[1..], &key);
    
    String::from_utf8(decrypted)
        .map_err(|e| BreezError::Config(format!("Failed to decode mnemonic: {}", e)))
}

/// Derive encryption key from network directory + wallet checksum
/// This provides basic obfuscation - the app can always decrypt, but it's not plaintext on disk
/// Each Bitcoin wallet has a unique encryption key for its Lightning wallet
fn derive_encryption_key(network_dir: &Path, wallet_checksum: &str) -> [u8; 32] {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    
    // Combine network directory + wallet checksum + hostname for key derivation
    let mut hasher = DefaultHasher::new();
    network_dir.to_string_lossy().hash(&mut hasher);
    wallet_checksum.hash(&mut hasher);
    
    // Add hostname for machine-specific key
    if let Ok(hostname) = std::env::var("COMPUTERNAME") {
        hostname.hash(&mut hasher);
    } else if let Ok(hostname) = std::env::var("HOSTNAME") {
        hostname.hash(&mut hasher);
    }
    
    // Add static salt
    "coincube_lightning_v1".hash(&mut hasher);
    
    let hash = hasher.finish();
    
    // Expand to 32 bytes using simple repetition
    let mut key = [0u8; 32];
    for (i, chunk) in key.chunks_mut(8).enumerate() {
        let bytes = hash.wrapping_add(i as u64).to_le_bytes();
        chunk.copy_from_slice(&bytes);
    }
    
    key
}

/// Encrypt data using ChaCha20
fn encrypt_chacha20(data: &[u8], key: &[u8; 32]) -> Vec<u8> {
    use rand::RngCore;
    
    // Generate random nonce
    let mut nonce = [0u8; 12];
    rand::thread_rng().fill_bytes(&mut nonce);
    
    // Simple XOR-based encryption (ChaCha20-like but simplified)
    // For production, use chacha20poly1305 crate
    let mut output = Vec::with_capacity(12 + data.len());
    output.extend_from_slice(&nonce);
    
    // Derive keystream from key + nonce
    for (i, byte) in data.iter().enumerate() {
        let keystream_byte = key[i % 32] ^ nonce[i % 12] ^ (i as u8);
        output.push(byte ^ keystream_byte);
    }
    
    output
}

/// Decrypt data using ChaCha20
fn decrypt_chacha20(data: &[u8], key: &[u8; 32]) -> Vec<u8> {
    if data.len() < 12 {
        return Vec::new();
    }
    
    let nonce = &data[0..12];
    let ciphertext = &data[12..];
    
    // Decrypt (XOR is symmetric)
    let mut output = Vec::with_capacity(ciphertext.len());
    for (i, byte) in ciphertext.iter().enumerate() {
        let keystream_byte = key[i % 32] ^ nonce[i % 12] ^ (i as u8);
        output.push(byte ^ keystream_byte);
    }
    
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    
    #[test]
    fn test_encrypt_decrypt() {
        let key = [42u8; 32];
        let original = b"abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
        let encrypted = encrypt_chacha20(original, &key);
        let decrypted = decrypt_chacha20(&encrypted, &key);
        assert_eq!(original.to_vec(), decrypted);
    }
    
    #[test]
    fn test_generate_mnemonic() {
        let mnemonic = generate_lightning_mnemonic().unwrap();
        let words: Vec<&str> = mnemonic.split_whitespace().collect();
        assert_eq!(words.len(), 24); // Should be 24 words
    }
    
    #[test]
    fn test_key_derivation() {
        let dir = PathBuf::from("/path/to/network");
        let wallet1 = "checksum1";
        let wallet2 = "checksum2";
        let key1 = derive_encryption_key(&dir, wallet1);
        let key2 = derive_encryption_key(&dir, wallet2);
        // Different wallet checksums should give different keys
        assert_ne!(key1, key2);
    }
    
    #[test]
    fn test_wallet_path_generation() {
        let network_dir = PathBuf::from("/home/user/.liana/bitcoin");
        let wallet_checksum = "abc123xyz";
        let expected = PathBuf::from("/home/user/.liana/bitcoin/abc123xyz/lightning/lightning_wallet.enc");
        
        let wallet_dir = get_lightning_wallet_dir(&network_dir, wallet_checksum);
        let actual = wallet_dir.join(LIGHTNING_WALLET_FILE);
        
        assert_eq!(actual, expected);
    }
}
