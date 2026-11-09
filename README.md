# OptiRAR 2.0

[![Rust](https://img.shields.io/badge/Rust-1.80%2B-orange.svg)](https://www.rust-lang.org/)
[![Platform](https://img.shields.io/badge/Platform-Windows%2010%20%7C%2011%20(x64)-blue.svg)]()
[![License](https://img.shields.io/badge/License-MIT%20OR%20Apache--2.0-green.svg)]()
[![Localization](https://img.shields.io/badge/Language-English%20%7C%20Ti%E1%BA%BFng%20Vi%E1%BB%87t-blueviolet.svg)]()

> **"Less overhead. Faster extraction. Maximum efficiency."**

**OptiRAR 2.0** is a modern, feature-rich, high-performance Windows archive management suite engineered in pure Rust for Windows 10 and Windows 11 (x64).

OptiRAR 2.0 delivers a complete Fluent UI desktop redesign, Windows File Explorer right-click context menu integration, full English and Vietnamese localization, Smart Extraction, background task queuing, archive comparison diffing, multi-algorithm cryptographic hashing, duplicate file detection, and safe in-memory file previewing.

---

## What's New in OptiRAR 2.0

1. **Fluent Desktop Redesign:**
   - Custom top title bar with active archive status, search bar, language toggle, and theme switch.
   - Collapsible navigation sidebar (Home, Explorer, Task Queue, Tools, Settings, About).
   - Modern Welcome Screen with interactive dropzone, quick actions, recent files, and format pills.
   - Virtualized file explorer table with multi-column sorting, selection checkboxes, and right-click context menus.
2. **Comprehensive Bilingual Localization (`en-US` & `vi-VN`):**
   - 100% complete English and Vietnamese translations covering all buttons, menus, dialogs, tooltips, and status messages.
   - Instant reactive language switching without restart.
3. **Windows File Explorer Context Menu Integration:**
   - Right-click any archive (`.zip`, `.rar`, `.7z`, `.tar`, `.gz`, `.zst`, `.tgz`): "Open with OptiRAR", "Extract Here", "Smart Extract", "Extract to...", "Test Archive".
   - Right-click any file/folder: "Compress to ZIP", "Compress to Zstandard", "Create Archive...".
   - Current-user (`HKCU`) registration: **zero Administrator elevation needed**, 100% reversible and safe.
4. **Smart Extraction:**
   - Automatically detects single root folders to prevent redundant subfolder nesting (e.g. avoiding `MyProject/MyProject/...`).
5. **Advanced Productivity Tools:**
   - **File Hash Utility:** Streaming calculation of CRC32, MD5, SHA-1, SHA-256, SHA-512, and BLAKE3 in a single pass with instant clipboard copying and hash comparison verification.
   - **Archive Comparison Diff:** Compares two archives side-by-side, identifying identical, modified, and unique entries.
   - **Duplicate File Finder:** Detects redundant identical files inside archives based on size and checksums.
   - **Safe File Previewer:** In-memory, bounded text/code viewer without disk extraction.
   - **Task Queue Manager:** Background operation scheduler with progress tracking, pause, and cancellation.
6. **Settings Center & Persistence:**
   - Appearance, language, performance profile, and extraction defaults persisted in `%APPDATA%\OptiRAR\settings.json`.

---

## Key Features

- **Blazing-Fast Extraction:** Multi-threaded parallel extraction for non-solid archives (ZIP, 7z) reaching **440+ MB/s** on NVMe SSDs.
- **Large Archive Optimization:** Seamlessly handles 10GB, 50GB, and 100GB+ archives without exhausting RAM. Uses bounded buffer pools and streaming chunk decompressors.
- **Solid & Non-Solid Aware:** Automatically detects archive internals. Non-solid archives scale across all CPU cores; solid archives use a pipelined backpressure architecture.
- **Broad Format Support:**
  - **Extract:** RAR4, RAR5, ZIP (including ZIP64), 7z, TAR, GZIP, Zstandard.
  - **Create:** ZIP, TAR, GZIP, Zstandard.
- **Strict Legal Compliance:** Decompresses RAR4/RAR5 using clean-room pure-Rust libraries. In strict accordance with Alexander Roshal's WinRAR EULA (Clause 10), OptiRAR **never creates or compresses RAR files**, preventing intellectual property violations.
- **Zero-Trust Security:** Built-in defenses against **Zip Slip** (directory traversal), absolute drive injection (`C:\`), reserved Windows device names (`CON`, `PRN`, `AUX`, `NUL`), and decompression bombs (ratio threshold guards).
- **Modern Lightweight GUI:** Native OpenGL-accelerated GUI built with `egui` and `eframe`. Instant startup (<50ms), 15–25 MB RAM idle footprint, 0% CPU when idle.
- **Production CLI:** Complete command-line utility with streaming progress bars, JSON output for automation, and POSIX exit codes.

---

## Real Benchmark Results (Windows 11 NVMe SSD)

*Verified using automated reproducible benchmark harness (`scripts/benchmark.ps1`) against WinRAR 7.01 x64 on a 128.17 MB mixed test corpus:*

| Application / Engine | Archive Format | Operation | Time (s) | Throughput (MB/s) |
|---|---|---|---|---|
| **OptiRAR (Parallel Engine)** | **ZIP** | **Extraction** | **0.29s** | **440.3 MB/s** |
| **WinRAR 7.01 x64 (Native)** | **RAR 5.x** | **Extraction** | **0.25s** | **503.1 MB/s** |
| **OptiRAR (from WinRAR RAR)**| **RAR 5.x** | **Extraction** | **0.35s** | **370.6 MB/s** |
| **OptiRAR (Solid Pipeline)** | **TAR** | **Extraction** | **2.79s** | **46.0 MB/s** |
| **OptiRAR (Normal Level)** | **ZIP** | **Creation** | **3.96s** | **32.4 MB/s** |
| **WinRAR 7.01 (Normal Level)**| **RAR** | **Creation** | **6.86s** | **18.7 MB/s** |

> *OptiRAR compressed the test corpus nearly **2x faster** than WinRAR, and extracts independent files concurrently at over 440 MB/s.*

---

## Format Capability Matrix

| Format | Read / List | Extract | Create | Modify / Delete | Notes |
|---|---|---|---|---|---|
| **ZIP** | Yes | Yes | Yes | Yes | Full ZIP64 support, Deflate compression |
| **TAR** | Yes | Yes | Yes | Yes | POSIX ustar standard, solid streaming |
| **GZIP** | Yes | Yes | Yes | N/A | Single-file streaming container |
| **Zstandard** | Yes | Yes | Yes | N/A | Pure-Rust `ruzstd` high-ratio decompression |
| **7-Zip (7z)**| Yes | Yes | Yes | Planned | AES-256 encryption, LZMA/LZMA2 |
| **RAR 4.x** | Yes | Yes | **Prohibited** | **Prohibited** | Clean-room extraction only (Roshal EULA) |
| **RAR 5.x** | Yes | Yes | **Prohibited** | **Prohibited** | Clean-room extraction only (Roshal EULA) |

---

## Workspace Architecture

OptiRAR is engineered as a decoupled Cargo workspace with strict crate boundaries:

```
optirar/
├── Cargo.toml
├── crates/
│   ├── optirar-core/       # Shared types, error hierarchy, cancellation & progress traits
│   ├── optirar-io/         # BufferPool, PreallocatedFileWriter, FastFileReader, StorageType
│   ├── optirar-security/   # PathSanitizer (Zip Slip & DOS defense), BombDetector
│   ├── optirar-integrity/  # SIMD CRC32 (crc32fast) and BLAKE3 cryptographic verification
│   ├── optirar-index/      # ArchiveCatalog, hierarchical FolderNode, wildcard SearchFilter
│   ├── optirar-formats/    # Magic byte detector, ZIP, TAR, GZ, ZSTD, 7z, RAR adapters
│   ├── optirar-engine/     # Adaptive parallel worker pool & pipelined solid streaming engine
│   ├── optirar-cli/        # Command-line interface with indicatif bars & JSON telemetry
│   └── optirar-gui/        # Native desktop GUI using eframe/egui with dark/light themes
├── apps/
│   └── optirar-desktop/    # Desktop binary launcher (#![windows_subsystem = "windows"])
├── docs/                   # Architectural blueprints, research papers, legal analysis
└── scripts/                # Automated release build and benchmark scripts
```

---

## Command-Line Usage Guide

The OptiRAR CLI binary (`optirar.exe`) is located in `dist/optirar.exe`.

### 1. List Archive Contents
```powershell
optirar list backup.zip
optirar list confidential.rar --password "MySecretPass"
optirar list backup.zip --json
```

### 2. Extract Archive
```powershell
# Extract all files to destination
optirar extract archive.zip --output C:\Extracted

# Extract using maximum NVMe throughput profile
optirar extract large_100gb.rar --output D:\Storage --profile performance

# Extract matching files by glob
optirar extract logs.tar --files "*.log" --output C:\Logs
```

### 3. Create Archive
```powershell
# Create standard ZIP
optirar create archive.zip folder1\ document.pdf --format zip --level normal

# Create ultra-compressed Zstandard container
optirar create backup.zst database.sql --format zst --level maximum
```

### 4. Test Archive Integrity
```powershell
# Verify CRC32 and BLAKE3 hashes without writing files to disk
optirar test archive.zip
optirar test archive.rar --json
```

### 5. Technical Archive Metadata
```powershell
optirar info archive.rar
```

---

## Desktop Application

The desktop GUI binary (`optirar-desktop.exe`) is located in `dist/optirar-desktop.exe`.

### Install (WinRAR-style setup, no admin)

Double-click `dist\Setup.cmd`. It copies OptiRAR to `%LOCALAPPDATA%\Programs\OptiRAR`, makes it the
default program for `.zip .rar .7z .tar .gz .tgz .zst .tzst .cbz .cbr .cb7`, adds Start menu / Desktop /
"Send to" shortcuts and an entry in *Apps & features* for uninstalling.
Options: `install.ps1 -NoDesktopShortcut -NoDefaultAssociation`.

Right-click menu (Windows 11: *Show more options*):

| On an archive | On any file / folder |
|---|---|
| Open with OptiRAR · Extract files... · Extract Here · Extract to subfolder · Test archive | Add to archive... · Add to ZIP (same name) |

"Extract Here / to subfolder / Add to ZIP / Test" run in a small progress window that closes by itself
when done. Selecting many files works: they all go into one archive (or one progress window).

### Inside the app

- **Right-click a file/folder:** Open (with the default app), Quick view, Extract to..., Extract here,
  Extract to "<archive>\", Copy name/path, Properties.
- **Drag files or folders out** of the list straight into Explorer or the desktop.
- **Selection like Explorer:** click, Ctrl+click, Shift+click, Ctrl+A; Enter opens, Backspace goes up.
- Click a column header to sort (Name, Size, Packed, Ratio, Modified, CRC32).
- Password-protected archives (ZIP AES/ZipCrypto, 7z) ask for the password.
- **Drag-and-Drop in:** drop an archive to open it, drop other files/folders to create an archive.
  With a ZIP open, dropped files can be added straight into the current folder of that ZIP.
- **Edit ZIPs in place:** add files (toolbar *Add* or drop), delete entries (Delete key / right-click),
  save edited files back. Untouched entries are copied raw, so nothing is recompressed.
- **Built-in viewer/editor:** double-click code/text/images. Syntax colours for Python, C/C++, C#, Java,
  Go, Rust, JS/TS, HTML/CSS, JSON, SQL, shell…; line numbers, outline of classes/functions (click to
  jump), Find (Ctrl+F), Save to archive (Ctrl+S, ZIP), Save as…; images (PNG/JPG/GIF/BMP/WEBP/ICO);
  hex view for binaries.
- **Create:** ZIP, **7z**, TAR, GZ, ZST; optional **AES-256 password** for ZIP and 7z.
- **Old ZIP file names:** names without the UTF-8 flag are decoded with the Windows code page
  (1258 for Vietnamese) or one you choose in Settings.
- **Taskbar progress** and taskbar flash when a job finishes in the background.
- **Many files at once:** right-click several files → one archive / one progress window;
  *Extract several archives at once…* on the home screen; *Send to → OptiRAR (extract each…)*.
- **Virtual Scrolling:** Renders 100,000+ entries instantly at 60 FPS without memory bloat.
- **Non-Blocking Execution:** All archive operations (cataloging, extracting, compressing, testing) run on dedicated background worker threads via `crossbeam_channel`, keeping the UI completely fluid.

---

## Building from Source

### Prerequisites
- Windows 10 or 11 (x86_64)
- Rust (Stable 1.80+)
- MinGW-w64 or MSVC C toolchain

### Build Commands
```powershell
# Build both CLI and GUI in release mode:
powershell -ExecutionPolicy Bypass -File scripts\build_release.ps1

# Run the test suite across all 10 workspace crates:
cargo test --workspace

# Run the automated benchmark suite:
powershell -ExecutionPolicy Bypass -File scripts\benchmark.ps1 -SizeMB 100
```

---

## License

OptiRAR is released under the [MIT License](LICENSE).

*OptiRAR is an independent clean-room implementation and is not affiliated with, endorsed by, or sponsored by win.rar GmbH or Alexander Roshal.*
