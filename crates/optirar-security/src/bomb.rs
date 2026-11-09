use optirar_core::error::{OptiError, Result};

/// Detects and defends against decompression bombs (Zip Bombs).
/// Protects system memory and disk from infinite expansion loops (e.g. 42.zip).
#[derive(Debug, Clone)]
pub struct BombDetector {
    /// Maximum allowed total uncompressed bytes across an entire extraction operation.
    pub max_uncompressed_bytes: u64,
    /// Maximum allowable expansion ratio (uncompressed / compressed).
    pub max_compression_ratio: f64,
    /// Minimum uncompressed bytes written before ratio enforcement triggers (avoids false positives on tiny sparse headers).
    pub min_bytes_before_ratio_check: u64,

    total_compressed: u64,
    total_uncompressed: u64,
}

impl Default for BombDetector {
    fn default() -> Self {
        Self {
            max_uncompressed_bytes: 500 * 1024 * 1024 * 1024, // 500 GB limit
            max_compression_ratio: 250.0,                     // 250:1 ratio limit
            min_bytes_before_ratio_check: 10 * 1024 * 1024,   // 10 MB threshold
            total_compressed: 0,
            total_uncompressed: 0,
        }
    }
}

impl BombDetector {
    pub fn new(max_uncompressed_bytes: u64, max_compression_ratio: f64) -> Self {
        Self {
            max_uncompressed_bytes,
            max_compression_ratio,
            ..Default::default()
        }
    }

    /// Update tracked bytes and evaluate bomb thresholds.
    pub fn record_bytes(&mut self, compressed: u64, uncompressed: u64) -> Result<()> {
        self.total_compressed += compressed;
        self.total_uncompressed += uncompressed;

        // Check absolute volume limit
        if self.total_uncompressed > self.max_uncompressed_bytes {
            return Err(OptiError::ArchiveBombDetected {
                ratio: self.current_ratio(),
                uncompressed_size: self.total_uncompressed,
            });
        }

        // Check ratio once sufficient volume has been expanded
        if self.total_uncompressed > self.min_bytes_before_ratio_check {
            let ratio = self.current_ratio();
            if ratio > self.max_compression_ratio {
                return Err(OptiError::ArchiveBombDetected {
                    ratio,
                    uncompressed_size: self.total_uncompressed,
                });
            }
        }

        Ok(())
    }

    /// Current uncompressed / compressed expansion ratio.
    pub fn current_ratio(&self) -> f64 {
        if self.total_compressed == 0 {
            if self.total_uncompressed == 0 {
                1.0
            } else {
                f64::INFINITY
            }
        } else {
            self.total_uncompressed as f64 / self.total_compressed as f64
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bomb_detector_normal() {
        let mut detector = BombDetector::default();
        // 10MB compressed expands to 50MB (5:1)
        assert!(detector
            .record_bytes(10 * 1024 * 1024, 50 * 1024 * 1024)
            .is_ok());
    }

    #[test]
    fn test_bomb_detector_catches_bomb() {
        let mut detector = BombDetector::default();
        // 100 KB compressed expands to 100 MB (1000:1 ratio, over 10MB min check)
        let res = detector.record_bytes(100 * 1024, 100 * 1024 * 1024);
        assert!(res.is_err());
    }
}
