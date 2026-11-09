use blake3::Hasher;

/// Vectorized SIMD cryptographic hashing using BLAKE3.
/// Capable of hashing at over 5 GB/s on modern AVX2 / AVX-512 processors.
#[derive(Debug, Clone, Default)]
pub struct Blake3Engine {
    hasher: Hasher,
}

impl Blake3Engine {
    pub fn new() -> Self {
        Self {
            hasher: Hasher::new(),
        }
    }

    /// Feed byte slice into hasher.
    pub fn update(&mut self, data: &[u8]) {
        self.hasher.update(data);
    }

    /// Return 64-character lowercase hex string of the 256-bit hash.
    pub fn finalize_hex(&self) -> String {
        self.hasher.clone().finalize().to_hex().to_string()
    }

    /// Compute BLAKE3 hex of a complete byte slice.
    pub fn compute_hex(data: &[u8]) -> String {
        let mut engine = Self::new();
        engine.update(data);
        engine.finalize_hex()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_blake3_computation() {
        let hex = Blake3Engine::compute_hex(b"OptiRAR High Performance Archive Engine");
        assert_eq!(hex.len(), 64);
    }
}
