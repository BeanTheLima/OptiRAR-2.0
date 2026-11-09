use clap::{Parser, Subcommand, ValueEnum};
use indicatif::{ProgressBar, ProgressStyle};
use optirar_core::traits::{CancellationToken, ProgressSnapshot};
use optirar_core::types::{
    format_bytes, ArchiveFormat, CompressionLevel, CompressionProfile, ExtractionOptions,
    OverwritePolicy,
};
use optirar_engine::compare::ArchiveComparator;
use optirar_engine::compressor::OptiCompressor;
use optirar_engine::extractor::OptiExtractor;
use optirar_engine::smart_extract::compute_smart_destination;
use optirar_formats::factory::open_reader;
use optirar_index::catalog::ArchiveCatalog;
use optirar_index::search::SearchFilter;
use optirar_integrity::MultiHasher;
use serde_json::json;
use std::path::{Path, PathBuf};
use std::process::exit;
use std::time::Instant;

#[derive(Parser, Debug)]
#[command(
    name = "optirar",
    about = "OptiRAR: High-performance, lightweight archive management utility for Windows",
    version,
    long_about = "OptiRAR is engineered in Rust for extreme extraction speed and large-file efficiency (10GB-100GB+).\nPhilosopy: Less overhead. Faster extraction. Maximum efficiency."
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// List archive contents
    List {
        /// Path to the archive file
        archive: PathBuf,
        /// Optional password for encrypted archives
        #[arg(short, long)]
        password: Option<String>,
        /// Output listing in JSON format
        #[arg(long)]
        json: bool,
    },

    /// Extract archive contents to destination
    Extract {
        /// Path to the archive file
        archive: PathBuf,
        /// Destination output directory (default: current directory)
        #[arg(short, long, default_value = ".")]
        output: PathBuf,
        /// Specific file or glob pattern to extract
        #[arg(short, long)]
        files: Option<String>,
        /// Optional decryption password
        #[arg(short, long)]
        password: Option<String>,
        /// Performance resource profile
        #[arg(long, value_enum, default_value_t = CliProfile::Balanced)]
        profile: CliProfile,
        /// Conflict resolution overwrite policy
        #[arg(long, value_enum, default_value_t = CliOverwrite::Overwrite)]
        overwrite: CliOverwrite,
        /// Keep partially extracted files on failure
        #[arg(long)]
        keep_broken: bool,
        /// Machine-readable JSON streaming progress
        #[arg(long)]
        json: bool,
    },

    /// Create a new archive in supported open format (ZIP, TAR, GZ, ZSTD)
    Create {
        /// Destination archive file path
        archive: PathBuf,
        /// Source files or directories to include
        #[arg(required = true)]
        sources: Vec<PathBuf>,
        /// Container format (zip, tar, gz, zst)
        #[arg(short, long)]
        format: Option<CliFormat>,
        /// Compression level
        #[arg(short, long, value_enum, default_value_t = CliLevel::Normal)]
        level: CliLevel,
        /// Output summary in JSON format
        #[arg(long)]
        json: bool,
    },

    /// Test archive integrity and verify checksums without writing to disk
    Test {
        /// Path to the archive file
        archive: PathBuf,
        /// Decryption password if required
        #[arg(short, long)]
        password: Option<String>,
        /// Output result in JSON format
        #[arg(long)]
        json: bool,
    },

    /// Display technical metadata and compression statistics
    Info {
        /// Path to the archive file
        archive: PathBuf,
        /// Decryption password if required
        #[arg(short, long)]
        password: Option<String>,
        /// Output metadata in JSON format
        #[arg(long)]
        json: bool,
    },

    /// Extract archive with Smart Extraction (prevents redundant subfolder nesting)
    SmartExtract {
        /// Path to the archive file
        archive: PathBuf,
        /// Base destination directory (default: parent of archive or current dir)
        #[arg(short, long, default_value = ".")]
        output: PathBuf,
        /// Optional password
        #[arg(short, long)]
        password: Option<String>,
        /// Overwrite policy
        #[arg(long, value_enum, default_value_t = CliOverwrite::Overwrite)]
        overwrite: CliOverwrite,
    },

    /// Compute cryptographic and integrity hashes for a file (CRC32, MD5, SHA-1, SHA-256, SHA-512, BLAKE3)
    Hash {
        /// Path to the file
        file: PathBuf,
        /// Output hashes in JSON format
        #[arg(long)]
        json: bool,
    },

    /// Compare two archives and report identical, modified, and unique files
    Compare {
        /// Path to first archive
        archive_a: PathBuf,
        /// Path to second archive
        archive_b: PathBuf,
        /// Output diff in JSON format
        #[arg(long)]
        json: bool,
    },

    /// Register Windows File Explorer right-click context menu integration (HKCU)
    RegisterShell,

    /// Unregister Windows File Explorer right-click context menu integration
    UnregisterShell,

    /// Run an end-to-end throughput benchmark on an archive
    Benchmark {
        /// Path to the archive file to benchmark
        archive: PathBuf,
        /// Output benchmark telemetry in JSON format
        #[arg(long)]
        json: bool,
    },
}

#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
enum CliProfile {
    Eco,
    Balanced,
    Performance,
}

impl From<CliProfile> for CompressionProfile {
    fn from(p: CliProfile) -> Self {
        match p {
            CliProfile::Eco => CompressionProfile::Eco,
            CliProfile::Balanced => CompressionProfile::Balanced,
            CliProfile::Performance => CompressionProfile::Performance,
        }
    }
}

#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
enum CliOverwrite {
    Overwrite,
    Skip,
    Error,
}

impl From<CliOverwrite> for OverwritePolicy {
    fn from(o: CliOverwrite) -> Self {
        match o {
            CliOverwrite::Overwrite => OverwritePolicy::Overwrite,
            CliOverwrite::Skip => OverwritePolicy::SkipExisting,
            CliOverwrite::Error => OverwritePolicy::ErrorOnConflict,
        }
    }
}

#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
enum CliLevel {
    Store,
    Fast,
    Normal,
    Maximum,
}

impl From<CliLevel> for CompressionLevel {
    fn from(l: CliLevel) -> Self {
        match l {
            CliLevel::Store => CompressionLevel::Store,
            CliLevel::Fast => CompressionLevel::Fastest,
            CliLevel::Normal => CompressionLevel::Normal,
            CliLevel::Maximum => CompressionLevel::Maximum,
        }
    }
}

#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
enum CliFormat {
    Zip,
    Tar,
    Gzip,
    Zstd,
}

impl From<CliFormat> for ArchiveFormat {
    fn from(f: CliFormat) -> Self {
        match f {
            CliFormat::Zip => ArchiveFormat::Zip,
            CliFormat::Tar => ArchiveFormat::Tar,
            CliFormat::Gzip => ArchiveFormat::Gzip,
            CliFormat::Zstd => ArchiveFormat::Zstd,
        }
    }
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::List {
            archive,
            password,
            json,
        } => handle_list(&archive, password.as_deref(), json),

        Commands::Extract {
            archive,
            output,
            files,
            password,
            profile,
            overwrite,
            keep_broken,
            json,
        } => handle_extract(
            &archive,
            &output,
            files.as_deref(),
            password.as_deref(),
            profile.into(),
            overwrite.into(),
            keep_broken,
            json,
        ),

        Commands::Create {
            archive,
            sources,
            format,
            level,
            json,
        } => handle_create(&archive, &sources, format, level.into(), json),

        Commands::Test {
            archive,
            password,
            json,
        } => handle_test(&archive, password.as_deref(), json),

        Commands::Info {
            archive,
            password,
            json,
        } => handle_info(&archive, password.as_deref(), json),

        Commands::Benchmark { archive, json } => handle_benchmark(&archive, json),

        Commands::SmartExtract {
            archive,
            output,
            password,
            overwrite,
        } => handle_smart_extract(&archive, &output, password.as_deref(), overwrite.into()),

        Commands::Hash { file, json } => handle_hash(&file, json),

        Commands::Compare {
            archive_a,
            archive_b,
            json,
        } => handle_compare(&archive_a, &archive_b, json),

        Commands::RegisterShell => handle_register_shell(),

        Commands::UnregisterShell => handle_unregister_shell(),
    }
}

fn handle_list(archive_path: &Path, password: Option<&str>, json_output: bool) {
    let mut reader = match open_reader(archive_path, password) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Error opening archive: {}", e);
            exit(1);
        }
    };

    let entries = match reader.list_entries() {
        Ok(e) => e,
        Err(e) => {
            eprintln!("Error reading catalog: {}", e);
            exit(1);
        }
    };

    let format = reader.format();
    let is_solid = reader.is_solid();
    let catalog = ArchiveCatalog::new(format, entries, is_solid);

    if json_output {
        println!("{}", serde_json::to_string_pretty(&catalog.entries).unwrap());
        return;
    }

    println!("Archive: {}", archive_path.display());
    println!("Format:  {} (Solid: {})", format.name(), if is_solid { "Yes" } else { "No" });
    println!("Entries: {}", catalog.len());
    println!("{:-<100}", "");
    println!(
        "{:<6} {:<12} {:<12} {:<8} {:<10} {}",
        "Index", "Size", "Packed", "Ratio", "Type", "Name"
    );
    println!("{:-<100}", "");

    for entry in &catalog.entries {
        let entry_type = if entry.is_directory { "DIR" } else { "FILE" };
        println!(
            "{:<6} {:<12} {:<12} {:<7.1}% {:<10} {}",
            entry.index,
            entry.display_size(),
            entry.display_compressed_size(),
            entry.compression_ratio(),
            entry_type,
            entry.path
        );
    }

    println!("{:-<100}", "");
    println!(
        "Total: {} files, {} dirs, {} uncompressed, {} packed ({:.1}% ratio)",
        catalog.file_count,
        catalog.dir_count,
        format_bytes(catalog.total_uncompressed_bytes),
        format_bytes(catalog.total_compressed_bytes),
        catalog.metadata().overall_ratio()
    );
}

fn handle_extract(
    archive_path: &Path,
    output_dir: &Path,
    files_filter: Option<&str>,
    password: Option<&str>,
    profile: CompressionProfile,
    overwrite: OverwritePolicy,
    keep_broken: bool,
    json_output: bool,
) {
    let mut reader = match open_reader(archive_path, password) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Error opening archive: {}", e);
            exit(1);
        }
    };

    let entries = match reader.list_entries() {
        Ok(e) => e,
        Err(e) => {
            eprintln!("Error reading entries: {}", e);
            exit(1);
        }
    };

    let selected_indices: Option<Vec<usize>> = if let Some(query) = files_filter {
        let catalog = ArchiveCatalog::new(reader.format(), entries, reader.is_solid());
        let searcher = SearchFilter::new(query);
        Some(searcher.search(&catalog))
    } else {
        None
    };

    let options = ExtractionOptions {
        output_dir: output_dir.to_path_buf(),
        overwrite_policy: overwrite,
        profile,
        selected_indices,
        preserve_attributes: true,
        preserve_timestamps: true,
        password: password.map(|s| s.to_string()),
        keep_broken,
        strip_prefix: None,
    };

    let cancel_token = CancellationToken::new();

    if json_output {
        let listener = move |snap: &ProgressSnapshot| {
            let data = json!({
                "files_completed": snap.files_completed,
                "total_files": snap.total_files,
                "bytes_completed": snap.bytes_completed,
                "total_bytes": snap.total_bytes,
                "throughput_bytes_per_sec": snap.throughput_bytes_per_sec,
                "percent": snap.percent(),
                "current_file": snap.current_file
            });
            println!("{}", data);
        };

        match OptiExtractor::extract(archive_path, options, cancel_token, Some(listener)) {
            Ok(summary) => {
                println!(
                    "{}",
                    json!({
                        "status": "success",
                        "files_extracted": summary.files_extracted,
                        "bytes_extracted": summary.bytes_extracted,
                        "elapsed_seconds": summary.elapsed.as_secs_f64(),
                        "throughput_mb_s": summary.average_throughput_mb_s,
                        "failed_count": summary.failed_entries.len()
                    })
                );
            }
            Err(e) => {
                eprintln!("{}", json!({"status": "error", "message": e.to_string()}));
                exit(1);
            }
        }
    } else {
        println!("Extracting '{}' to '{}'", archive_path.display(), output_dir.display());
        println!("Workers: {} | Buffer: {} KB", profile.worker_threads(), profile.buffer_size_bytes() / 1024);

        let pb = ProgressBar::new(100);
        pb.set_style(
            ProgressStyle::default_bar()
                .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {percent}% ({msg}) | {eta}")
                .unwrap()
                .progress_chars("#>-"),
        );

        let pb_clone = pb.clone();
        let listener = move |snap: &ProgressSnapshot| {
            pb_clone.set_position(snap.percent() as u64);
            pb_clone.set_message(format!(
                "{} | {}",
                snap.display_throughput(),
                snap.current_file
            ));
        };

        match OptiExtractor::extract(archive_path, options, cancel_token, Some(listener)) {
            Ok(summary) => {
                pb.finish_with_message("Done!");
                println!("\nExtraction completed successfully!");
                println!("Files extracted:   {}", summary.files_extracted);
                println!("Total size:        {}", format_bytes(summary.bytes_extracted));
                println!("Elapsed time:      {:.2}s", summary.elapsed.as_secs_f64());
                println!("Average Throughput:{:.1} MB/s", summary.average_throughput_mb_s);

                if !summary.failed_entries.is_empty() {
                    println!("\nWarning: {} entries failed to extract:", summary.failed_entries.len());
                    for (file, err) in &summary.failed_entries {
                        println!("  {} -> {}", file, err);
                    }
                }
            }
            Err(e) => {
                pb.abandon_with_message("Failed");
                eprintln!("\nExtraction failed: {}", e);
                exit(1);
            }
        }
    }
}

fn handle_create(
    archive_path: &Path,
    sources: &[PathBuf],
    format_arg: Option<CliFormat>,
    level: CompressionLevel,
    json_output: bool,
) {
    let format = match format_arg {
        Some(f) => f.into(),
        None => ArchiveFormat::from_path(archive_path),
    };

    if !format.can_create() {
        eprintln!(
            "Error: Creating {} archives is not supported or prohibited by license.",
            format.name()
        );
        eprintln!("OptiRAR supports creating ZIP, TAR, GZ, and ZSTD archives.");
        exit(1);
    }

    let cancel_token = CancellationToken::new();

    if json_output {
        match OptiCompressor::create_archive(archive_path, sources, format, level, cancel_token, None::<fn(&ProgressSnapshot)>) {
            Ok(summary) => {
                println!(
                    "{}",
                    json!({
                        "status": "success",
                        "files_added": summary.files_added,
                        "bytes_compressed": summary.bytes_compressed,
                        "archive_size": summary.archive_size_bytes,
                        "elapsed_seconds": summary.elapsed.as_secs_f64(),
                        "throughput_mb_s": summary.average_throughput_mb_s
                    })
                );
            }
            Err(e) => {
                eprintln!("{}", json!({"status": "error", "message": e.to_string()}));
                exit(1);
            }
        }
    } else {
        println!("Creating {} archive: '{}'", format.name(), archive_path.display());
        let pb = ProgressBar::new_spinner();
        pb.set_message("Compressing...");

        let pb_clone = pb.clone();
        let listener = move |snap: &ProgressSnapshot| {
            pb_clone.set_message(format!(
                "Adding {} ({})",
                snap.current_file,
                snap.display_throughput()
            ));
        };

        match OptiCompressor::create_archive(archive_path, sources, format, level, cancel_token, Some(listener)) {
            Ok(summary) => {
                pb.finish_with_message("Done!");
                println!("\nArchive created successfully!");
                println!("Files added:       {}", summary.files_added);
                println!("Original volume:   {}", format_bytes(summary.bytes_compressed));
                println!("Archive file size: {}", format_bytes(summary.archive_size_bytes));
                println!("Elapsed time:      {:.2}s", summary.elapsed.as_secs_f64());
                println!("Throughput:        {:.1} MB/s", summary.average_throughput_mb_s);
            }
            Err(e) => {
                pb.abandon_with_message("Failed");
                eprintln!("\nCreation failed: {}", e);
                exit(1);
            }
        }
    }
}

fn handle_test(archive_path: &Path, password: Option<&str>, json_output: bool) {
    let cancel_token = CancellationToken::new();

    if !json_output {
        println!("Testing archive integrity: '{}'", archive_path.display());
    }

    match OptiExtractor::test_archive(archive_path, password, cancel_token) {
        Ok(summary) => {
            if json_output {
                println!(
                    "{}",
                    json!({
                        "status": if summary.failed_entries.is_empty() { "ok" } else { "failed" },
                        "files_tested": summary.files_extracted,
                        "bytes_tested": summary.bytes_extracted,
                        "errors": summary.failed_entries
                    })
                );
            } else if summary.failed_entries.is_empty() {
                println!("Integrity check passed! All {} files verified (CRC32/BLAKE3 OK).", summary.files_extracted);
                println!("Tested volume: {}", format_bytes(summary.bytes_extracted));
            } else {
                eprintln!("Integrity verification FAILED with {} errors:", summary.failed_entries.len());
                for (file, err) in &summary.failed_entries {
                    eprintln!("  {} -> {}", file, err);
                }
                exit(2);
            }
        }
        Err(e) => {
            if json_output {
                println!("{}", json!({"status": "error", "message": e.to_string()}));
            } else {
                eprintln!("Error testing archive: {}", e);
            }
            exit(1);
        }
    }
}

fn handle_info(archive_path: &Path, password: Option<&str>, json_output: bool) {
    let mut reader = match open_reader(archive_path, password) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Error: {}", e);
            exit(1);
        }
    };

    let entries = match reader.list_entries() {
        Ok(e) => e,
        Err(e) => {
            eprintln!("Error reading archive: {}", e);
            exit(1);
        }
    };

    let format = reader.format();
    let is_solid = reader.is_solid();
    let catalog = ArchiveCatalog::new(format, entries, is_solid);
    let meta = catalog.metadata();

    if json_output {
        println!("{}", serde_json::to_string_pretty(&meta).unwrap());
        return;
    }

    println!("========================================");
    println!("OptiRAR Technical Archive Information");
    println!("========================================");
    println!("Archive Path:       {}", archive_path.display());
    println!("Container Format:   {}", meta.format.name());
    println!("Solid Block Stream: {}", if meta.is_solid { "Yes (Sequential dependency)" } else { "No (Parallel random access)" });
    println!("Encrypted:          {}", if meta.is_encrypted { "Yes (Password protected)" } else { "No" });
    println!("Total Entries:      {}", meta.total_entries);
    println!("Total Files:        {}", meta.file_count);
    println!("Total Folders:      {}", meta.dir_count);
    println!("Uncompressed Size:  {} ({} bytes)", format_bytes(meta.uncompressed_size), meta.uncompressed_size);
    println!("Compressed Size:    {} ({} bytes)", format_bytes(meta.compressed_size), meta.compressed_size);
    println!("Compression Ratio:  {:.2}%", meta.overall_ratio());
    println!("64-bit Addressable: Yes (ZIP64 / 64-bit RAR5)");
    println!("========================================");
}

fn handle_benchmark(archive_path: &Path, json_output: bool) {
    if !json_output {
        println!("Running OptiRAR High-Performance Benchmark on: '{}'", archive_path.display());
        println!("Measuring raw decompressor throughput & working memory...");
    }

    let cancel_token = CancellationToken::new();
    let start = Instant::now();

    match OptiExtractor::test_archive(archive_path, None, cancel_token) {
        Ok(summary) => {
            let elapsed = start.elapsed();
            if json_output {
                println!(
                    "{}",
                    json!({
                        "benchmark": "OptiRAR Raw Decompression",
                        "archive": archive_path.display().to_string(),
                        "format": summary.archive_format.name(),
                        "files": summary.files_extracted,
                        "bytes": summary.bytes_extracted,
                        "duration_seconds": elapsed.as_secs_f64(),
                        "throughput_mb_s": summary.average_throughput_mb_s
                    })
                );
            } else {
                println!("\nBenchmark Results:");
                println!("----------------------------------------");
                println!("Format:          {}", summary.archive_format.name());
                println!("Files processed: {}", summary.files_extracted);
                println!("Data decompressed: {}", format_bytes(summary.bytes_extracted));
                println!("Total Wall Time: {:.3}s", elapsed.as_secs_f64());
                println!("Peak Throughput: {:.2} MB/s", summary.average_throughput_mb_s);
                println!("----------------------------------------");
            }
        }
        Err(e) => {
            eprintln!("Benchmark failed: {}", e);
            exit(1);
        }
    }
}

fn handle_smart_extract(
    archive_path: &Path,
    output_base: &Path,
    password: Option<&str>,
    overwrite: OverwritePolicy,
) {
    let mut reader = match open_reader(archive_path, password) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Error opening archive: {}", e);
            exit(1);
        }
    };

    let entries = match reader.list_entries() {
        Ok(e) => e,
        Err(e) => {
            eprintln!("Error reading archive entries: {}", e);
            exit(1);
        }
    };

    let target_dest = compute_smart_destination(archive_path, output_base, &entries);
    println!(
        "Smart Extraction -> Target: {}",
        target_dest.display()
    );

    let options = ExtractionOptions {
        output_dir: target_dest,
        overwrite_policy: overwrite,
        profile: CompressionProfile::Balanced,
        selected_indices: None,
        preserve_attributes: true,
        preserve_timestamps: true,
        password: password.map(|s| s.to_string()),
        keep_broken: false,
        strip_prefix: None,
    };

    let cancel_token = CancellationToken::new();
    let pb = ProgressBar::new(entries.iter().map(|e| e.uncompressed_size).sum());
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta}) {msg}")
            .unwrap()
            .progress_chars("#>-")
    );

    let pb_clone = pb.clone();
    let listener = move |snapshot: &ProgressSnapshot| {
        pb_clone.set_position(snapshot.bytes_completed);
        pb_clone.set_message(format!("{:.1} MB/s - {}", snapshot.throughput_bytes_per_sec / (1024.0 * 1024.0), snapshot.current_file));
    };

    match OptiExtractor::extract(archive_path, options, cancel_token, Some(listener)) {
        Ok(summary) => {
            pb.finish_with_message("Done!");
            println!("\nSmart Extraction complete:");
            println!("  Files extracted: {}", summary.files_extracted);
            println!("  Bytes written:   {}", format_bytes(summary.bytes_extracted));
            println!("  Duration:        {:.2}s", summary.elapsed.as_secs_f64());
            println!("  Throughput:      {:.2} MB/s", summary.average_throughput_mb_s);
        }
        Err(e) => {
            pb.abandon();
            eprintln!("Extraction failed: {}", e);
            exit(1);
        }
    }
}

fn handle_hash(file_path: &Path, json_output: bool) {
    if !file_path.exists() {
        eprintln!("Error: File '{}' does not exist.", file_path.display());
        exit(1);
    }

    match MultiHasher::compute_all(file_path, None) {
        Ok(hashes) => {
            if json_output {
                println!("{}", serde_json::to_string_pretty(&hashes).unwrap());
            } else {
                println!("\nFile Hash Analysis: {}", hashes.file_path.display());
                println!("{:-<70}", "");
                println!("File Size:   {} ({} bytes)", format_bytes(hashes.file_size), hashes.file_size);
                println!("Time:        {:.3}s ({:.1} MB/s)", hashes.elapsed_seconds, hashes.throughput_mb_s);
                println!("{:-<70}", "");
                println!("CRC32:       {}", hashes.crc32);
                println!("MD5:         {}", hashes.md5);
                println!("SHA-1:       {}", hashes.sha1);
                println!("SHA-256:     {}", hashes.sha256);
                println!("SHA-512:     {}", hashes.sha512);
                println!("BLAKE3:      {}", hashes.blake3);
                println!("{:-<70}", "");
            }
        }
        Err(e) => {
            eprintln!("Failed to compute hashes: {}", e);
            exit(1);
        }
    }
}

fn handle_compare(archive_a: &Path, archive_b: &Path, json_output: bool) {
    let mut reader_a = match open_reader(archive_a, None) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Error reading {}: {}", archive_a.display(), e);
            exit(1);
        }
    };
    let entries_a = match reader_a.list_entries() {
        Ok(e) => e,
        Err(e) => {
            eprintln!("Failed to list entries in {}: {}", archive_a.display(), e);
            exit(1);
        }
    };

    let mut reader_b = match open_reader(archive_b, None) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Error reading {}: {}", archive_b.display(), e);
            exit(1);
        }
    };
    let entries_b = match reader_b.list_entries() {
        Ok(e) => e,
        Err(e) => {
            eprintln!("Failed to list entries in {}: {}", archive_b.display(), e);
            exit(1);
        }
    };

    let diff = ArchiveComparator::compare(&entries_a, &entries_b);

    if json_output {
        println!("{}", serde_json::to_string_pretty(&diff).unwrap());
    } else {
        println!("\nArchive Comparison:");
        println!("  Archive A: {}", archive_a.display());
        println!("  Archive B: {}", archive_b.display());
        println!("{:-<70}", "");
        println!("Identical files: {}", diff.identical.len());
        println!("Modified files:  {}", diff.modified.len());
        println!("Only in A:       {}", diff.only_in_a.len());
        println!("Only in B:       {}", diff.only_in_b.len());
        println!("{:-<70}", "");

        if diff.is_identical() {
            println!("Result: Archives have identical contents!");
        } else {
            println!("Result: Found {} differences.", diff.total_differences());
            if !diff.modified.is_empty() {
                println!("\nModified Entries:");
                for (a, b) in &diff.modified {
                    println!("  ~ {} (A: {}, CRC:{:08X?} vs B: {}, CRC:{:08X?})",
                        a.path,
                        format_bytes(a.uncompressed_size),
                        a.crc32,
                        format_bytes(b.uncompressed_size),
                        b.crc32
                    );
                }
            }
            if !diff.only_in_a.is_empty() {
                println!("\nOnly in Archive A:");
                for a in &diff.only_in_a {
                    println!("  - {} ({})", a.path, format_bytes(a.uncompressed_size));
                }
            }
            if !diff.only_in_b.is_empty() {
                println!("\nOnly in Archive B:");
                for b in &diff.only_in_b {
                    println!("  + {} ({})", b.path, format_bytes(b.uncompressed_size));
                }
            }
        }
    }
}

fn handle_register_shell() {
    let script = include_str!("../../../scripts/register_shell.ps1");
    let temp_script = std::env::temp_dir().join("optirar_register_shell.ps1");
    if let Err(e) = std::fs::write(&temp_script, script) {
        eprintln!("Failed to create temporary script: {}", e);
        exit(1);
    }

    let current_exe = std::env::current_exe().unwrap_or_default();
    let current_dir = current_exe.parent().unwrap_or(Path::new(""));
    let gui_exe = current_dir.join("optirar-desktop.exe");

    let status = std::process::Command::new("powershell")
        .args(&[
            "-NoProfile",
            "-ExecutionPolicy",
            "Bypass",
            "-File",
            temp_script.to_str().unwrap(),
            "-OptiRarCli",
            current_exe.to_str().unwrap(),
            "-OptiRarGui",
            gui_exe.to_str().unwrap(),
        ])
        .status();

    let _ = std::fs::remove_file(&temp_script);

    match status {
        Ok(s) if s.success() => println!("OptiRAR shell integration registered successfully!"),
        Ok(s) => eprintln!("Failed to register shell integration: exited with code {:?}", s.code()),
        Err(e) => eprintln!("Failed to run PowerShell registration: {}", e),
    }
}

fn handle_unregister_shell() {
    let script = include_str!("../../../scripts/unregister_shell.ps1");
    let temp_script = std::env::temp_dir().join("optirar_unregister_shell.ps1");
    if let Err(e) = std::fs::write(&temp_script, script) {
        eprintln!("Failed to create temporary script: {}", e);
        exit(1);
    }

    let status = std::process::Command::new("powershell")
        .args(&["-NoProfile", "-ExecutionPolicy", "Bypass", "-File", temp_script.to_str().unwrap()])
        .status();

    let _ = std::fs::remove_file(&temp_script);

    match status {
        Ok(s) if s.success() => println!("OptiRAR shell integration unregistered successfully!"),
        Ok(s) => eprintln!("Failed to unregister shell integration: exited with code {:?}", s.code()),
        Err(e) => eprintln!("Failed to run PowerShell unregistration: {}", e),
    }
}
