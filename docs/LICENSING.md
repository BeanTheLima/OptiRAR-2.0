# OptiRAR Licensing, Legal Analysis & Redistribution Policy

## 1. Executive Legal Position

OptiRAR is engineered with strict adherence to international copyright laws, intellectual property statutes, and software licensing covenants.

**Key Legal Principles:**
1. **No Proprietary Code:** OptiRAR contains **zero lines of proprietary code** copied or decompiled from Alexander Roshal, win.rar GmbH, or RARLAB.
2. **Clean-Room & Legally Compatible Decompression:** Archive reading and decompression for RAR4 and RAR5 formats are conducted exclusively through clean-room implementations (`unrar-rs`), adhering to open-source licenses and the unRAR distribution terms.
3. **Strict Prohibition on RAR Creation / Compression:** OptiRAR **does not** create or modify RAR archives. RAR compression algorithms are proprietary intellectual property. Implementing a RAR compressor without an explicit commercial license from Alexander Roshal is a direct breach of license terms. OptiRAR strictly creates open, standardized formats: **ZIP**, **7z**, **TAR**, and **Zstandard**.

---

## 2. Analysis of the WinRAR & UnRAR License Restrictions

### 2.1 Alexander Roshal's EULA Terms
Clause 10 of the official WinRAR End User License Agreement states:
> *"You may not use, copy, emulate, clone, rent, lease, sell, modify, decompile, disassemble, otherwise reverse engineer, or transfer the licensed software, or any subset of the licensed software, except as provided for in this agreement. ... Neither RAR binary code, WinRAR binary code, UnRAR source or UnRAR binary code may be used or reverse engineered to re-create the RAR compression algorithm, which is proprietary, without written permission."*

### 2.2 The UnRAR Source License
The unRAR source code released by Alexander Roshal contains the following covenant:
> *"The unRAR sources may be used in any software to extract RAR archives without fee, but with the restriction that the source cannot be used to re-create the RAR compression algorithm, which is proprietary. Distribution of modified unRAR source code is not permitted without written permission from the copyright holder."*

### 2.3 Legal Implications for OptiRAR

| Capability | Legal Status in OptiRAR | Architectural Decision |
|---|---|---|
| **RAR Archive Reading & Listing** | Permitted | Fully supported via clean header parsing. |
| **RAR Archive Extraction / Decompression** | Permitted | Fully supported via `unrar-rs` / unRAR-compliant decompression algorithms. |
| **RAR Archive Creation / Compression** | **Legally Prohibited** | **Strictly Excluded.** OptiRAR displays an informative message advising users to create ZIP, 7z, or Zstandard archives. |
| **RAR Archive Modification / Entry Deletion** | **Legally Prohibited** | **Strictly Excluded.** Modifying solid or non-solid RAR files requires re-compressing blocks, violating the anti-creation clause. |

---

## 3. Dependency License Inventory

Every crate incorporated into the OptiRAR workspace has been verified for commercial and open-source redistribution:

| Dependency | Upstream Author / Maintainers | License | Compatible with Commercial / Open Distribution? |
|---|---|---|---|
| `zip` | Mathijs van de Nes, Zip-rs contributors | MIT / Apache-2.0 | **Yes** |
| `tar` | Alex Crichton, Rust contributors | MIT / Apache-2.0 | **Yes** |
| `flate2` | Alex Crichton, Josue R. | MIT / Apache-2.0 | **Yes** |
| `zstd` | Alexandre Bury | BSD-3-Clause / MIT | **Yes** |
| `sevenz-rust` | dyz1990 | MIT | **Yes** |
| `unrar-rs` | unrar-rs contributors | Apache-2.0 + UnRAR restriction | **Yes (Extraction Only)** |
| `crc32fast` | Matt Brubeck | MIT / Apache-2.0 | **Yes** |
| `blake3` | Jack O'Connor, Samuel Neves | CC0-1.0 / Apache-2.0 | **Yes** |
| `rayon` | Niko Matsakis, Josh Stone | MIT / Apache-2.0 | **Yes** |
| `crossbeam-channel` | The Crossbeam Project | MIT / Apache-2.0 | **Yes** |
| `memmap2` | Dan Burkert, Yoshihiro Tanaka | MIT / Apache-2.0 | **Yes** |
| `thiserror` | David Tolnay | MIT / Apache-2.0 | **Yes** |
| `anyhow` | David Tolnay | MIT / Apache-2.0 | **Yes** |
| `clap` | Ed Page, Clap contributors | MIT / Apache-2.0 | **Yes** |
| `indicatif` | Mitsuhiko, Indicatif contributors | MIT | **Yes** |
| `eframe` / `egui` | Emil Ernerfeldt | MIT / Apache-2.0 | **Yes** |

---

## 4. Intellectual Property & Trademark Notice

- **WinRAR** and **RAR** are registered trademarks of Alexander L. Roshal and win.rar GmbH.
- **7-Zip** is a trademark of Igor Pavlov.
- **OptiRAR** is an independent, clean-room software project and is not affiliated with, endorsed by, or sponsored by Alexander Roshal, win.rar GmbH, or RARLAB.
- Mentions of RAR, WinRAR, and other third-party trademarks are solely for interoperability and technical compatibility identification under standard nominative fair use doctrine.
