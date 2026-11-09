# OptiRAR Dependency Decision Matrix & Ecosystem Evaluation

## 1. Dependency Decision Matrix

To ensure stability, extreme performance, zero licensing violations, and predictable maintenance, every external dependency in the OptiRAR project has been rigorously evaluated across five criteria:
1. **Performance & Overhead:** Minimal memory footprint, high throughput, low CPU overhead.
2. **Maintenance & Community:** Actively maintained with healthy crate governance and prompt security patches.
3. **Safety & Unsafe Code Footprint:** Low or zero unnecessary `unsafe` blocks.
4. **Licensing:** Legally compatible with commercial, proprietary, and open-source redistribution (MIT, Apache-2.0, BSD-3-Clause, Zlib).
5. **Suitability:** Direct fit for large-file archive processing without architectural compromises.

| Crate | Version | License | Decision | Justification & Architectural Role |
|---|---|---|---|---|
| `zip` | 2.2+ / 9.x | MIT / Apache-2.0 | **Adopted** | The industry standard pure-Rust ZIP reader/writer. Full support for ZIP64 (handles >4GB files), Deflate, Bzip2, Zstd, and AES encryption. |
| `tar` | 0.4+ | MIT / Apache-2.0 | **Adopted** | Fast, streaming TAR archive reader/writer. No external C dependencies; ideal for streaming pipelines. |
| `flate2` | 1.0+ | MIT / Apache-2.0 | **Adopted** | Fast DEFLATE and GZIP compression/decompression. Uses `miniz_oxide` pure-Rust backend by default with optional C `zlib-ng` acceleration. |
| `zstd` | 0.13+ | BSD-3-Clause / MIT | **Adopted** | Ultra-high performance modern compression algorithm. Provides multi-threaded compression and 1.5–3 GB/s decompression. |
| `sevenz-rust` | 0.6+ | MIT | **Adopted** | Pure Rust implementation of 7z archive decompression (LZMA, LZMA2, BCJ filters, AES-256). Avoids proprietary C++ 7-Zip DLLs. |
| `unrar-rs` | 0.15+ | Apache-2.0 + UnRAR restriction | **Adopted (Isolated)** | High quality pure-Rust implementation of RAR4/RAR5 decompression, header parsing, and streaming extraction. Isolated strictly to the extraction layer. |
| `crc32fast` | 1.4+ | MIT / Apache-2.0 | **Adopted** | Hardware-accelerated CRC32 using Intel SSE4.2 / ARM CRC32 instructions with automatic scalar fallback. Reaches 10+ GB/s throughput. |
| `blake3` | 1.5+ | CC0-1.0 / Apache-2.0 | **Adopted** | Fastest cryptographic hash function available. Vectorized AVX-512/AVX2/NEON implementation reaching >5 GB/s per core. |
| `rayon` | 1.10+ | MIT / Apache-2.0 | **Adopted** | Work-stealing CPU parallelism engine for concurrent file extraction in non-solid archives and multi-threaded compression. |
| `crossbeam-channel` | 0.5+ | MIT / Apache-2.0 | **Adopted** | High-performance multi-producer multi-consumer bounded channels. Powers the backpressure pipeline between decompressor and disk writers. |
| `memmap2` | 0.9+ | MIT / Apache-2.0 | **Adopted (Selective)** | Fast, cross-platform memory mapping. Evaluated and restricted to index reading on fast NVMe drives; excluded from general 100 GB streaming decompression to avoid page cache thrashing. |
| `thiserror` | 1.0 / 2.0 | MIT / Apache-2.0 | **Adopted** | Zero-cost ergonomic error derivation for library crates (`optirar-core`, `optirar-formats`). |
| `anyhow` | 1.0+ | MIT / Apache-2.0 | **Adopted (App layer)** | Flexible error handling for application binary crates (`optirar-cli`, `optirar-desktop`). |
| `clap` | 4.5+ | MIT / Apache-2.0 | **Adopted (CLI)** | Modern, fast command-line argument parser with `derive` macros and shell completion generation. |
| `indicatif` | 0.17+ | MIT | **Adopted (CLI)** | High-performance, flicker-free terminal progress bars and spinning throughput indicators. |
| `eframe` / `egui` | 0.29+ / 0.31+ | MIT / Apache-2.0 | **Adopted (GUI)** | Immediate-mode pure Rust desktop GUI. Starts in under 50ms, uses only 15–25 MB RAM, 0% CPU at idle, zero Chromium/WebView2 overhead. |
| `tokio` | 1.x | MIT | **Rejected for Core Engine** | Asynchronous runtimes add unnecessary task switching and future state machine overhead for synchronous CPU-bound decompression and sequential file I/O. Rayon + Crossbeam channels provide higher raw throughput with lower latency. |
| `bytes` | 1.6+ | MIT | **Adopted (Selective)** | Zero-copy byte buffer slices used in network/disk streaming boundaries. |
| `lz4_flex` | 0.11+ | MIT | **Adopted (Candidate)** | Pure-Rust LZ4 implementation for ultra-fast intermediate caching. |
| `lzma-rs` | 0.3+ | MIT | **Adopted (Candidate)** | Pure-Rust XZ and LZMA stream decompressor for standalone `.xz` files. |

---

## 2. Evaluation of Evaluated and Rejected Libraries

### Why `tokio` was not selected for the core archive engine:
Archive decompression is predominantly **CPU-bound** (dictionary matching, entropy decoding, bit unpacking) and **sequential disk I/O bound**. Async runtimes like `tokio` excel at managing tens of thousands of idle network sockets. For archive extraction, thread pool work-stealing via `rayon` combined with dedicated bounded I/O worker threads yields:
- Predictable CPU utilization (1 worker per physical core).
- Zero asynchronous state machine memory overhead.
- Direct synchronous Win32 file streaming with optimal sequential read-ahead hints.

### Why `Tauri` was rejected in favor of `eframe` (egui):
- **Tauri:** Uses Microsoft WebView2 (Chromium engine). Cold start takes 400ms–1.2s. Memory baseline is 100 MB–250 MB RAM. Requires IPC serialization across JSON-RPC bridges between JavaScript and Rust for every progress update.
- **eframe / egui:** 100% native Rust GUI rendering directly via DirectX / WGPU. Cold start is instant (<50ms). Memory baseline is 15 MB–25 MB RAM. Progress updates occur in-process with zero serialization overhead. Perfect alignment with OptiRAR's philosophy: *"Less overhead. Faster extraction. Maximum efficiency."*
