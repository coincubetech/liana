use aes_gcm::{aead::{Aead, KeyInit, OsRng}, Aes256Gcm, Nonce};
use argon2::{password_hash::{PasswordHasher, SaltString}, Argon2};
use bip39::{Mnemonic, Language, Seed};
use rand::RngCore;
use std::{
    fs::{self, File},
    io::{Read, Write},
    path::{Path, PathBuf},
};
use zeroize::Zeroize;

const NONCE_LEN: usize = 12; // AES-GCM standard nonce
const SALT_LEN: usize = 16;
pub const MNEMONICS_FOLDER_NAME: &str = "mnemonics";

pub struct EncryptedSigner {
    pub cube_id: String,
    pub mnemonic: Mnemonic,
    pub seed: Seed,
}

impl EncryptedSigner {
    fn base_dir() -> PathBuf {
        dirs::home_dir()
            .unwrap()
            .join(".liana")
            .join("mnemonics")
    }

    fn file_path(cube_id: &str) -> PathBuf {
        Self::base_dir().join(format!("{}.bin", cube_id))
    }

    /// Generate a new encrypted mnemonic for a cube
    pub fn generate(cube_id: &str, password: &str) -> std::io::Result<Self> {
        fs::create_dir_all(Self::base_dir())?;

        let mnemonic = Mnemonic::generate_in(Language::English, 12).unwrap();
        let seed = Seed::new(&mnemonic, "");

        let salt = SaltString::generate(&mut rand::thread_rng());
        let password_hasher = Argon2::default();
        let key = password_hasher
            .hash_password(password.as_bytes(), &salt)
            .unwrap()
            .hash
            .unwrap();

        let key_bytes = hex::decode(key.as_str()).unwrap();
        let cipher = Aes256Gcm::new_from_slice(&key_bytes).unwrap();

        let mut nonce_bytes = [0u8; NONCE_LEN];
        OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        let ciphertext = cipher
            .encrypt(nonce, mnemonic.phrase().as_bytes())
            .unwrap();

        let path = Self::file_path(cube_id);
        let mut file = File::create(path)?;
        file.write_all(salt.as_bytes())?;
        file.write_all(&nonce_bytes)?;
        file.write_all(&ciphertext)?;

        Ok(Self {
            cube_id: cube_id.to_string(),
            mnemonic,
            seed,
        })
    }

    /// Load an encrypted mnemonic for a cube
    pub fn load(cube_id: &str, password: &str) -> std::io::Result<Self> {
        let path = Self::file_path(cube_id);
        let mut file = File::open(path)?;
        let mut buffer = vec![];
        file.read_to_end(&mut buffer)?;

        let salt_bytes = &buffer[..SALT_LEN];
        let nonce_bytes = &buffer[SALT_LEN..SALT_LEN + NONCE_LEN];
        let ciphertext = &buffer[SALT_LEN + NONCE_LEN..];

        let salt = SaltString::b64_encode(salt_bytes).unwrap();
        let password_hasher = Argon2::default();
        let key = password_hasher
            .hash_password(password.as_bytes(), &salt)
            .unwrap()
            .hash
            .unwrap();
        let key_bytes = hex::decode(key.as_str()).unwrap();
        let cipher = Aes256Gcm::new_from_slice(&key_bytes).unwrap();

        let plaintext = cipher
            .decrypt(Nonce::from_slice(nonce_bytes), ciphertext)
            .unwrap();
        let phrase = String::from_utf8(plaintext).unwrap();

        let mnemonic = Mnemonic::from_phrase(&phrase, Language::English).unwrap();
        let seed = Seed::new(&mnemonic, "");

        let mut key_bytes_clone = key_bytes.clone();
        key_bytes_clone.zeroize();

        Ok(Self {
            cube_id: cube_id.to_string(),
            mnemonic,
            seed,
        })
    }

    /// List all known cube IDs
    pub fn list_cubes() -> std::io::Result<Vec<String>> {
        let dir = Self::base_dir();
        if !dir.exists() {
            return Ok(vec![]);
        }

        let mut cubes = vec![];
        for entry in fs::read_dir(dir)? {
            let path = entry?.path();
            if let Some(name) = path.file_stem().and_then(|s| s.to_str()) {
                cubes.push(name.to_string());
            }
        }
        Ok(cubes)
    }

    /// Check if a cube already exists
    pub fn exists(cube_id: &str) -> bool {
        Self::file_path(cube_id).exists()
    }
}