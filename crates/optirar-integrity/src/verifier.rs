use crate::blake3_hash::Blake3Engine;
use crate::crc32::Crc32Engine;
use optirar_core::error::{OptiError, Result};

/// Stream-aware checksum verifier. Computes CRC32 and BLAKE3 during writing.
#[derive(Debug, Clone, Default)]
pub struct ChecksumVerifier {
    crc: Crc32Engine,
    blake3: Blake3Engine,
}

impl ChecksumVerifier {
    pub fn new() -> Self {
        Self::default()
    }

    /// Update with incoming uncompressed chunk.
    pub fn update(&mut self, chunk: &[u8]) {
        self.crc.update(chunk);
        self.blake3.update(chunk);
    }

    /// Computed CRC32 integer.
    pub fn computed_crc32(&self) -> u32 {
        self.crc.finalize()
    }

    /// Computed BLAKE3 lowercase hex string.
    pub fn computed_blake3_hex(&self) -> String {
        self.blake3.finalize_hex()
    }

    /// Validate against optional expected CRC32.
    pub fn verify_crc32(&self, file_name: &str, expected_crc: Option<u32>) -> Result<()> {
        if let Some(expected) = expected_crc {
            let actual = self.computed_crc32();
            if actual != expected {
                return Err(OptiError::ChecksumMismatch {
                    file: file_name.to_string(),
                    expected: format!("0x{:08X}", expected),
                    actual: format!("0x{:08X}", actual),
                });
            }
        }
        Ok(())
    }
}
