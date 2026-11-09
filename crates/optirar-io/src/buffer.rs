use std::ops::{Deref, DerefMut};
use std::sync::{Arc, Mutex};

/// A thread-safe pool of reusable memory buffers.
/// Minimizes heap allocation overhead and prevents fragmentation during continuous streaming.
#[derive(Debug, Clone)]
pub struct BufferPool {
    inner: Arc<BufferPoolInner>,
}

#[derive(Debug)]
struct BufferPoolInner {
    chunk_size: usize,
    max_buffers: usize,
    available: Mutex<Vec<Vec<u8>>>,
}

impl BufferPool {
    /// Create a new buffer pool with the given chunk size in bytes and maximum pool capacity.
    pub fn new(chunk_size: usize, max_buffers: usize) -> Self {
        Self {
            inner: Arc::new(BufferPoolInner {
                chunk_size,
                max_buffers,
                available: Mutex::new(Vec::with_capacity(max_buffers)),
            }),
        }
    }

    /// Default pool with 256 KB chunks and up to 32 buffers.
    pub fn default_pool() -> Self {
        Self::new(256 * 1024, 32)
    }

    /// Lease a buffer from the pool. If none are available, allocates a new one of `chunk_size`.
    pub fn acquire(&self) -> PooledBuffer {
        let mut available = self.inner.available.lock().unwrap();
        let buffer = if let Some(mut buf) = available.pop() {
            buf.resize(self.inner.chunk_size, 0);
            buf
        } else {
            vec![0u8; self.inner.chunk_size]
        };

        PooledBuffer {
            buffer: Some(buffer),
            pool: Arc::clone(&self.inner),
        }
    }

    /// Number of available buffers currently in the pool.
    pub fn available_count(&self) -> usize {
        self.inner.available.lock().unwrap().len()
    }

    /// Buffer chunk size in bytes.
    pub fn chunk_size(&self) -> usize {
        self.inner.chunk_size
    }
}

/// An RAII handle to a buffer leased from a `BufferPool`.
/// Returns the buffer back to the pool automatically when dropped.
pub struct PooledBuffer {
    buffer: Option<Vec<u8>>,
    pool: Arc<BufferPoolInner>,
}

impl Deref for PooledBuffer {
    type Target = [u8];
    fn deref(&self) -> &Self::Target {
        self.buffer.as_ref().unwrap()
    }
}

impl DerefMut for PooledBuffer {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.buffer.as_mut().unwrap()
    }
}

impl AsRef<[u8]> for PooledBuffer {
    fn as_ref(&self) -> &[u8] {
        self.buffer.as_ref().unwrap()
    }
}

impl AsMut<[u8]> for PooledBuffer {
    fn as_mut(&mut self) -> &mut [u8] {
        self.buffer.as_mut().unwrap()
    }
}

impl Drop for PooledBuffer {
    fn drop(&mut self) {
        if let Some(buf) = self.buffer.take() {
            let mut available = self.pool.available.lock().unwrap();
            if available.len() < self.pool.max_buffers {
                available.push(buf);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_buffer_pool_reuse() {
        let pool = BufferPool::new(1024, 4);
        assert_eq!(pool.available_count(), 0);

        {
            let mut buf1 = pool.acquire();
            assert_eq!(buf1.len(), 1024);
            buf1[0] = 42;
        }
        // Dropped buf1 returned to pool
        assert_eq!(pool.available_count(), 1);

        {
            let buf2 = pool.acquire();
            assert_eq!(buf2.len(), 1024);
            assert_eq!(pool.available_count(), 0);
        }
        assert_eq!(pool.available_count(), 1);
    }
}
