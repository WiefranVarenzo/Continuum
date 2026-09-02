use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::{CoreError, Result};

pub(crate) fn relative_content_path(hash: &str) -> Result<PathBuf> {
    if hash.len() != 64 || !hash.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(CoreError::Validation("invalid SHA-256 digest".into()));
    }
    Ok(PathBuf::from("artifacts")
        .join("sha256")
        .join(&hash[0..2])
        .join(&hash[2..4])
        .join(hash))
}

pub(crate) fn hash_file(path: &Path) -> Result<(String, u64)> {
    let mut file = fs::File::open(path)?;
    let mut digest = Sha256::new();
    let mut size = 0_u64;
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        size += read as u64;
        digest.update(&buffer[..read]);
    }
    Ok((hex::encode(digest.finalize()), size))
}
