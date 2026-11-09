# OptiRAR Compatibility & Format Capability Matrix

## 1. Format Capability Matrix

The following table specifies OptiRAR's format support across all lifecycle operations (Read/Index, Extract, Create, and Modify).

| Format | Extension(s) | Read / List | Extract | Create | Modify / Append | Technical Implementation |
|---|---|---|---|---|---|---|
| **ZIP** | `.zip` | **Full** | **Full** | **Full** | **Supported** | Pure Rust (`zip` crate with ZIP64, Deflate, Zstd, Bzip2, AES) |
| **TAR** | `.tar` | **Full** | **Full** | **Full** | **Supported** | Pure Rust (`tar` crate, POSIX ustar / GNU format) |
| **GZIP** | `.gz`, `.tgz` | **Full** | **Full** | **Full** | **N/A (Stream)** | Pure Rust (`flate2`, Deflate streaming wrapper) |
| **Zstandard** | `.zst`, `.tar.zst` | **Full** | **Full** | **Full** | **Supported (via TAR)** | Pure Rust / C-accelerated (`zstd`, multi-threaded) |
| **7z** | `.7z` | **Full** | **Full** | **Supported** | **Read-Only Archive** | Pure Rust (`sevenz-rust`, LZMA/LZMA2, AES-256) |
| **RAR 4.x** | `.rar` | **Full** | **Full** | **Prohibited by License** | **Prohibited by License** | Pure Rust (`unrar-rs`, LZSS + Huffman decompression) |
| **RAR 5.x** | `.rar` | **Full** | **Full** | **Prohibited by License** | **Prohibited by License** | Pure Rust (`unrar-rs`, vint headers, BLAKE2sp, AES-256) |
| **BZIP2** | `.bz2`, `.tbz2` | **Full** | **Full** | **Full** | **N/A (Stream)** | Pure Rust (`bzip2` streaming wrapper) |
| **XZ** | `.xz`, `.txz` | **Full** | **Full** | **Full** | **N/A (Stream)** | Pure Rust (`lzma-rs` / `xz2` streaming wrapper) |

---

## 2. Windows Environment & Filesystem Compatibility

OptiRAR is engineered specifically for Windows 10 (version 1809+) and Windows 11 on `x86_64` (AMD64) architecture.

### 2.1 Filesystem Support

| Filesystem | Max File Size | Max Volume Size | Hard Links | Symlinks / Junctions | Alternate Data Streams (ADS) | Sparse Files |
|---|---|---|---|---|---|---|
| **NTFS** | 16 TB (standard) – 8 PB | Up to 8 PB | Supported | Supported | Supported | Supported |
| **exFAT** | 16 EB | 128 PB | Not Supported | Not Supported | Not Supported | Not Supported |
| **FAT32** | 4 GB - 1 byte | 2 TB (Windows format) | Not Supported | Not Supported | Not Supported | Not Supported |
| **ReFS** | 16 EB | 4 YB | Supported | Supported | Supported | Supported |

### 2.2 Win32 Path Limits and the `\\?\` Extended-Length Prefix
- Standard Windows Win32 API imposes a `MAX_PATH` constraint of 260 characters (`D:\very\long\path...`).
- Large archives frequently contain deeply nested directory hierarchies exceeding 260 characters.
- **OptiRAR Strategy:**
  - OptiRAR automatically canonicalizes output target paths using the Windows extended-length path prefix: `\\?\UNC\` for network shares, or `\\?\` for local drive volumes (e.g., `\\?\C:\Extraction\VeryLongPath...`).
  - This extends the maximum path limit to **32,767 characters**, completely eliminating path-length truncation errors during extraction.

### 2.3 Unicode & Character Encoding Handling
- Modern archives use UTF-8 filenames (ZIP flag bit 11, RAR5 UTF-8 vints, 7z UTF-16LE, TAR Pax headers).
- Legacy archives may contain filenames encoded in DOS code pages (e.g., CP437, CP850) or regional Windows ANSI code pages (CP1252, Windows-932/Shift-JIS, Windows-949/EUC-KR).
- **OptiRAR Strategy:**
  - Native path representation uses Rust's UTF-8 `Path` / `PathBuf` internally, converted to UTF-16 (`LPCWSTR`) for Win32 OS calls.
  - Automatic fallback decoding: If a byte stream in an archive header is invalid UTF-8, OptiRAR attempts OEM/ANSI codepage decoding or lossy replacement, preventing unhandled decoding crashes.

### 2.4 File Attributes and Timestamps
- **Timestamps:** Preserves Modification Time (`mtime`), Creation Time (`ctime`), and Access Time (`atime`) with standard 100-nanosecond precision Windows `FILETIME` structures.
- **Attributes:** Preserves Windows standard attributes:
  - `FILE_ATTRIBUTE_READONLY` (`0x0001`)
  - `FILE_ATTRIBUTE_HIDDEN` (`0x0002`)
  - `FILE_ATTRIBUTE_SYSTEM` (`0x0004`)
  - `FILE_ATTRIBUTE_ARCHIVE` (`0x0020`)
- Security note: Dangerous attributes like `FILE_ATTRIBUTE_TEMPORARY` or hidden system flags on root extracted targets are sanitized unless explicitly configured by the user.

---

## 3. Large-File & ZIP64 Specifications

### 3.1 Standard ZIP Limits vs. ZIP64
- Standard ZIP headers are constrained by 32-bit integer fields:
  - Maximum uncompressed file size: $2^{32} - 1 \approx 4.29\text{ GB}$.
  - Maximum compressed file size: $2^{32} - 1 \approx 4.29\text{ GB}$.
  - Maximum archive total size: $4.29\text{ GB}$.
  - Maximum number of files: $2^{16} - 1 = 65,535$.
- **OptiRAR ZIP64 Implementation:**
  - Seamlessly activates ZIP64 format when creating archives exceeding 4 GB or 65,535 entries.
  - Automatically parses ZIP64 Extended Information Extra Field (`0x0001`) during extraction and listing.
  - Prevents 32-bit truncation bugs and handles 10 GB, 50 GB, and 100 GB+ archives without integer wrap-around.

### 3.2 Multi-Volume Archive Support
- **RAR Multi-Volume:**
  - Modern naming: `archive.part01.rar`, `archive.part02.rar`, ...
  - Legacy naming: `archive.rar`, `archive.r00`, `archive.r01`, ...
  - OptiRAR automatically detects and links multi-volume sequences, streaming file extraction seamlessly across volume boundaries.
- **7z Multi-Volume:** `archive.7z.001`, `archive.7z.002`, ...
- **ZIP Split Archives:** `archive.zip`, `archive.z01`, `archive.z02`, ...

---

## 4. Solid Archive Extraction Constraints & Handling

| Archive Type | Random Access Extraction | Parallel Extraction Support | Optimal Extraction Method |
|---|---|---|---|
| **Non-Solid ZIP** | Instant ($O(1)$ seek) | **Yes** (Concurrent worker threads) | Worker pool with range reads |
| **Non-Solid 7z** | Instant ($O(1)$ seek) | **Yes** (Concurrent worker threads) | Parallel extraction per folder |
| **Non-Solid RAR** | Instant ($O(1)$ seek) | **Yes** (Concurrent worker threads) | Parallel file streaming |
| **Solid 7z** | Linear ($O(N)$ within block) | **No** (Decompression must be serial) | Pipelined decompressor $\to$ async disk writer |
| **Solid RAR4/RAR5** | Linear ($O(N)$ within block) | **No** (Decompression must be serial) | Pipelined decompressor $\to$ async disk writer |
| **TAR / TAR.GZ / TAR.ZST** | Linear ($O(N)$ stream) | **No** (Single continuous stream) | Stream decompressor $\to$ parallel directory unpacker |

OptiRAR inspects the archive header flags prior to extraction and dynamically routes the operation to either the **Parallel Worker Pool Engine** or the **Pipelined Stream Engine**.
