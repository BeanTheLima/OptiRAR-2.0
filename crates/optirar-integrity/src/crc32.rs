use crc32fast::Hasher;

/// High-throughput hardware-accelerated CRC32 checksum engine.
/// Utilizes Intel SSE4.2 and ARM CRC32 SIMD instructions where supported.
#[derive(Debug, Clone, Default)]
pub struct Crc32Engine {
    hasher: Hasher,
}

impl Crc32Engine {
    pub fn new() -> Self {
        Self {
            hasher: Hasher::new(),
        }
    }

    /// Update checksum with a slice of bytes.
    pub fn update(&mut self, data: &[u8]) {
        self.hasher.update(data);
    }

    /// Finalize and return computed CRC32 value.
    pub fn finalize(&self) -> u32 {
        self.hasher.clone().finalize()
    }

    /// Compute CRC32 of a complete byte slice in a single call.
    pub fn compute(data: &[u8]) -> u32 {
        let mut engine = Self::new();
        engine.update(data);
        engine.finalize()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_crc32_computation() {
        // Standard check: CRC32 of "123456789" is 0xCBF43926 (3421780262)
        let data = b"123456789";
        let crc = Crc32Engine::compute(data);
        assert_eq!(crc, 0xCBF43926);
    }
}
