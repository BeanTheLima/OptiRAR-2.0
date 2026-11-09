# OptiRAR Comprehensive Benchmark Plan & Methodology

## 1. Objectives & Principles

The benchmark suite is designed to measure real-world systems throughput, memory footprint, and CPU efficiency of OptiRAR compared directly against **WinRAR 7.01** and **7-Zip 24.x**.

### Core Benchmarking Principles
1. **Zero Fabrication:** Never estimate or fabricate benchmark data. All reported metrics must be directly collected from automated test scripts on identical hardware.
2. **Cold vs. Warm Cache Testing:** Measure cold cache (system reboot or file cache flush) and warm cache (OS page cache preloaded) to distinguish raw storage I/O limits from decompression CPU efficiency.
3. **Reproducibility:** Provide automated PowerShell and Rust benchmark scripts (`scripts/benchmark.ps1`, `benches/`) to allow anyone to verify results.

---

## 2. Test Hardware & Environment Specification

- **OS:** Windows 11 Pro 64-bit (Version 23H2 / 24H2)
- **CPU:** Multi-core x86_64 processor (measure across 4-core, 8-core, 16-core threads)
- **RAM:** Minimum 16 GB DDR4/DDR5
- **Storage Devices Tested:**
  - Fast NVMe PCIe 4.0 SSD (Sequential read > 5000 MB/s, write > 4000 MB/s)
  - SATA III SSD (Sequential ~500 MB/s)
  - Standard Mechanical HDD (Sequential ~150 MB/s, high seek latency)

---

## 3. Workload Categories

| Workload ID | Name | Archive Size | File Composition | Compression Behavior | Target Stress Area |
|---|---|---|---|---|---|
| **W-100M** | Small Corpus | ~100 MB | 5,000 files (1 KB – 50 KB) | Highly compressible (C/Rust source code, JSON, logs) | Directory traversal, file metadata, thread dispatch latency |
| **W-1G-ISO** | Single Large File | 1 GB | 1 monolithic file (Linux ISO / binary blob) | Moderate compressibility | Continuous streaming throughput, buffer pool reuse |
| **W-5G-MIX** | Medium Mixed Corpus | 5 GB | 25,000 files (mixed 10 KB to 50 MB) | Mixed (documents, binaries, media) | Adaptive thread scheduling, buffer memory bounds |
| **W-10G-BIG** | Large Enterprise Corpus | 10 GB | 10 files (1 GB each) | High / Moderate | 64-bit integer handling, memory ceiling under sustained load |
| **W-50G-EXT** | Massive Storage Test | 50 GB | 100,000 files | Mixed | Physical RAM stability, zero-copy I/O, no memory leakage |
| **W-SOLID** | Solid Archive Test | 1 GB / 5 GB | 10,000 files | Solid RAR5 and Solid 7z | Sequential decompression pipeline, bounded channel backpressure |
| **W-NONSOLID** | Non-Solid Parallel Test | 1 GB / 5 GB | 10,000 files | Non-solid ZIP and Non-solid RAR | Multi-threaded worker pool scaling ($N$ cores) |

---

## 4. Measured Performance Metrics

For every test run, the benchmark engine captures:
1. **Total Elapsed Time ($T$):** High-precision wall-clock time in milliseconds via `std::time::Instant`.
2. **Throughput ($\text{MB/s}$):**
   $$\text{Throughput} = \frac{\text{Total Uncompressed Megabytes}}{T_{\text{seconds}}}$$
3. **Peak Memory Consumption (Peak RSS / Working Set in MB):** Sampled via Windows API `GetProcessMemoryInfo` (`WorkingSetSize` and `PeakWorkingSetSize`).
4. **CPU Utilization (%):** Average and peak CPU usage across all logical cores via `GetProcessTimes`.
5. **Startup Time to UI ($T_{\text{start}}$):** Time in milliseconds from process launch to first frame rendered / CLI command ready.
6. **Archive Listing Time ($T_{\text{list}}$):** Time required to parse the central directory / archive headers and populate the catalog.
7. **Time to First Extracted File ($T_{\text{first}}$):** Latency from user clicking "Extract" until the first file is committed to disk.

---

## 5. Comparison Baseline Tools

- **OptiRAR (Release build, `--profile release`, LTO enabled)**
- **WinRAR 7.01 x64 (`Rar.exe` / `WinRAR.exe`)**
  - Command: `rar x -o+ -inul archive.rar target_dir\`
- **7-Zip 24.x x64 (`7z.exe`)**
  - Command: `7z x -y -bso0 -bsp0 archive.zip -otarget_dir\`

---

## 6. Automated Benchmark Script (`scripts/benchmark.ps1`)

The automated script creates synthetic test corpora, compresses them with 7-Zip, WinRAR, and OptiRAR, runs 5 warm-up iterations followed by 10 measured runs, logs raw JSON telemetry, and calculates mean and standard deviation.
