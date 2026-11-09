# OptiRAR System Architecture & Technical Specifications

## 1. High-Level Workspace Architecture

OptiRAR is engineered as a highly modular, decoupled Cargo workspace. The core archive engine has zero dependencies on graphical libraries, ensuring that the engine can be thoroughly unit-tested, benchmarked, embedded in CLI applications, and executed across headless servers or desktop environments.

```
optirar/
├── Cargo.toml                      (Workspace manifest)
├── Cargo.lock
├── crates/
│   ├── optirar-core/               (Core types, ArchiveFormat, Entry, Trait definitions, Errors)
│   ├── optirar-io/                 (High-throughput streaming I/O, RingBuffer, BufferPool, Storage detection)
│   ├── optirar-security/           (Zip Slip protection, canonicalization, archive bomb detection, sandboxing)
│   ├── optirar-integrity/          (SIMD Checksums: CRC32, BLAKE3, SHA-256 verification)
│   ├── optirar-index/              (Fast in-memory archive index, Trie search, directory hierarchy tree)
│   ├── optirar-formats/            (Format readers/writers: ZIP, TAR, GZIP, Zstandard, 7z, RAR)
│   ├── optirar-engine/             (Parallel extraction, solid pipelined engine, progress tracking, backpressure)
│   ├── optirar-cli/                (Full-featured command-line interface with JSON streaming & progress bars)
│   └── optirar-gui/                (eframe/egui desktop application library, reactive rendering, dark/light themes)
├── apps/
│   └── optirar-desktop/            (Main Windows desktop executable binary)
├── benches/                        (Criterion and custom throughput benchmarks)
├── tests/                          (Integration and fuzz test corpus)
├── docs/                           (Technical specifications, licensing, architecture, research)
└── scripts/                        (Build, package, and benchmark automation scripts)
```

---

## 2. Crate Responsibilities & Interfaces

### 2.1 `optirar-core`
- **Core Domain Entities:**
  - `ArchiveFormat`: Enumeration representing `Zip`, `Tar`, `Gzip`, `Zstd`, `SevenZip`, `Rar4`, `Rar5`, `Unknown`. Detectable by file extension or magic byte inspection.
  - `ArchiveEntry`: Metadata record representing a file or directory within an archive:
    - Path (normalized UTF-8 relative path).
    - Uncompressed size (64-bit integer, preventing 4 GB overflow).
    - Compressed size (64-bit integer).
    - Modified timestamp, Creation timestamp.
    - Unix permissions / Windows file attributes.
    - CRC32 / BLAKE3 checksum hashes.
    - Flags: `is_dir`, `is_encrypted`, `is_solid`.
  - `CompressionProfile`: `Eco` (low memory/threads), `Balanced` (optimal default), `Performance` (max IO/cores), `Custom`.
  - `OptiError` and `Result<T>`: Centralized error handling using `thiserror`.

### 2.2 `optirar-io`
- **Streaming Pipeline & Buffer Management:**
  - `BufferPool`: Thread-safe pool of reusable memory buffers (64 KB to 1 MB). Prevents heap allocations during hot extraction loops.
  - `FastReader`: High-throughput buffered reader utilizing Windows `FILE_FLAG_SEQUENTIAL_SCAN` hints for high sequential read speeds on NVMe and SATA SSDs.
  - `BoundedWriter`: Thread-safe file writer with pre-allocation (`SetFileInformationByHandle` allocation size) to minimize disk fragmentation on NTFS.
  - `StorageType`: Heuristic detection of underlying storage medium (NVMe SSD, SATA SSD, Spinning HDD, Network UNC path) to calibrate buffer sizes and thread counts automatically.

### 2.3 `optirar-security`
- **Zero-Trust Hardening:**
  - `PathSanitizer`: Validates that every archive entry path is relative, rejects `../` and Windows volume specifiers (`C:`), strips leading separators, and resolves target canonical paths inside the extraction root.
  - `BombDetector`: Tracks cumulative uncompressed bytes and compression ratios dynamically. Throws `OptiError::ArchiveBombDetected` if ratio exceeds 250:1 or total expanded size exceeds the user threshold.
  - `SafeOverwrite`: Implements overwrite policies: `Overwrite`, `SkipExisting`, `RenameNew`, `ErrorOnConflict`.

### 2.4 `optirar-integrity`
- **SIMD Checksum Engine:**
  - `Crc32Engine`: Hardware-accelerated CRC32 using Intel SSE4.2 / ARM CRC32 instructions via `crc32fast`.
  - `Blake3Engine`: Multi-threaded SIMD cryptographic hashing via `blake3` (used for RAR5 BLAKE2sp verification and internal integrity checks).
  - Streaming verification during extraction: Checksums are computed on-the-fly while bytes are written to disk, avoiding extra read passes.

### 2.5 `optirar-index`
- **Archive Catalog & Quick Search:**
  - `ArchiveIndex`: In-memory metadata database storing the complete file list.
  - `HierarchyTree`: Hierarchical folder node tree for instant folder navigation in the GUI without quadratic path scanning.
  - `SearchEngine`: High-speed string search supporting wildcards, regex, and sub-string matches across millions of entries in milliseconds.

### 2.6 `optirar-formats`
- **Format Abstraction Traits:**
  - `trait ArchiveReader: Send + Sync`:
    - `fn list_entries(&mut self) -> Result<Vec<ArchiveEntry>>;`
    - `fn extract_entry(&mut self, entry_index: usize, writer: &mut dyn Write) -> Result<()>;`
    - `fn is_solid(&self) -> bool;`
    - `fn format(&self) -> ArchiveFormat;`
  - `trait ArchiveWriter: Send`:
    - `fn add_file(&mut self, archive_path: &str, source_path: &Path) -> Result<()>;`
    - `fn finish(self: Box<Self>) -> Result<()>;`
  - Implementations:
    - `ZipFormat`: Full read, extract, create, and append.
    - `TarFormat`: Full read, extract, create.
    - `GzipFormat`: Streaming single-file wrapper.
    - `ZstdFormat`: High-throughput Zstandard compression and extraction.
    - `SevenZipFormat`: Pure Rust 7z reader and extractor.
    - `RarFormat`: Pure Rust RAR4/RAR5 reader and extractor via `unrar-rs`.

### 2.7 `optirar-engine`
- **Extraction Orchestration & Parallelism:**
  - `ExtractionEngine`: Main coordinator managing extraction jobs.
  - **Adaptive Execution Modes:**
    1. *Non-Solid Parallel Mode:* Dispatches file extraction jobs across a Rayon worker pool. Each thread extracts independent files concurrently.
    2. *Solid Pipelined Mode:* Uses a single decompression worker thread streaming uncompressed bytes into a bounded crossbeam channel, feeding a pool of asynchronous disk writer workers.
  - `ProgressTracker`: Atomic progress reporting (files completed, bytes processed, current throughput in MB/s, estimated time remaining).
  - `CancellationToken`: Thread-safe atomic flag (`Arc<AtomicBool>`) allowing instant, clean cancellation without leaving orphaned file handles.

### 2.8 `optirar-cli`
- Command-line binary providing subcommands:
  - `list`: Display tabular or JSON archive contents.
  - `extract`: High-speed extraction with interactive progress bar.
  - `create`: Create new ZIP, TAR, GZ, or ZSTD archives.
  - `test`: Integrity test validating CRC32/BLAKE3 without writing to disk.
  - `info`: Detailed technical metadata and compression statistics.
  - `benchmark`: Built-in throughput benchmark on arbitrary archives.

### 2.9 `optirar-gui` & `optirar-desktop`
- Lightweight, modern Windows GUI application built with `eframe` (egui):
  - Zero web-view dependencies (pure native Rust and DirectX/OpenGL/WGPU).
  - Dark and Light modern themes with high-contrast technical aesthetic.
  - Asynchronous background worker threads communicate with GUI via lock-free channels (`crossbeam_channel`).
  - Zero GUI freezes: UI message loop never blocks on disk I/O or archive decompression.
  - Real-time extraction progress window with accurate throughput (MB/s), file count, and pause/cancel buttons.

---

## 3. Data Flow Diagram

```
+-------------------------------------------------------------+
|               User Interaction (CLI or GUI)                 |
+-------------------------------------------------------------+
                              |
                              v
+-------------------------------------------------------------+
|                     optirar-engine                          |
|  - Inspects format & solid flag                             |
|  - Configures BufferPool based on StorageProfile            |
|  - Spawns CancellationToken & ProgressTracker              |
+-------------------------------------------------------------+
         |                                           |
         v (If Non-Solid)                            v (If Solid)
+---------------------------+               +---------------------------+
|   Parallel Worker Pool    |               | Pipelined Stream Engine   |
| (Rayon $N$ Core Workers)  |               |  (Single Decompressor)    |
+---------------------------+               +---------------------------+
         |                                           |
         |                                           v
         |                                  Bounded Channel (RingBuffer)
         |                                           |
         +--------------------+----------------------+
                              |
                              v
+-------------------------------------------------------------+
|                    optirar-security                         |
|  - PathSanitizer::sanitize(entry_path)                      |
|  - BombDetector::validate(ratio, size)                      |
+-------------------------------------------------------------+
                              |
                              v
+-------------------------------------------------------------+
|                    optirar-integrity                        |
|  - On-the-fly SIMD CRC32 / BLAKE3 check                     |
+-------------------------------------------------------------+
                              |
                              v
+-------------------------------------------------------------+
|                       optirar-io                            |
|  - Pre-allocated Windows File Handles                       |
|  - Chunked Bounded Disk Writing                             |
+-------------------------------------------------------------+
```

---

## 4. Error Propagation & Safety Model

- All fallible operations return standard Rust `Result<T, OptiError>`.
- The engine guarantees **no partial corrupt writes on failure**: If an error or cancellation occurs mid-extraction, incomplete files are immediately cleaned up and deleted unless the `--keep-broken` flag is explicitly requested.
- Memory safety: Entire engine runs in 100% safe Rust, with unsafe blocks restricted strictly to verified Win32 API calls (`SetFileInformationByHandle`, extended paths) and SIMD instruction intrinsics within well-audited crates (`crc32fast`, `blake3`).
