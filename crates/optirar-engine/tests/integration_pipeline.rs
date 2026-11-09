use optirar_core::traits::CancellationToken;
use optirar_core::types::{
    ArchiveFormat, CompressionLevel, CompressionProfile, ExtractionOptions, OverwritePolicy,
};
use optirar_engine::compressor::OptiCompressor;
use optirar_engine::extractor::OptiExtractor;
use optirar_formats::factory::open_reader;
use std::fs;
use tempfile::tempdir;

#[test]
fn test_zip_roundtrip_extraction() {
    let dir = tempdir().unwrap();
    let src_dir = dir.path().join("src");
    fs::create_dir(&src_dir).unwrap();

    let file1 = src_dir.join("file1.txt");
    let file2 = src_dir.join("file2.txt");
    let file3 = src_dir.join("subfolder").join("file3.txt");
    fs::create_dir_all(file3.parent().unwrap()).unwrap();

    fs::write(&file1, "First test file content for OptiRAR").unwrap();
    fs::write(&file2, "Second test file content with longer text data for deflate compression testing").unwrap();
    fs::write(&file3, "Nested subfolder file content").unwrap();

    let archive_path = dir.path().join("test_archive.zip");
    let sources = vec![file1.clone(), file2.clone(), file3.clone()];

    // 1. Create Archive
    let comp_summary = OptiCompressor::create_archive(
        &archive_path,
        &sources,
        ArchiveFormat::Zip,
        CompressionLevel::Normal,
        CancellationToken::new(),
        None::<fn(&optirar_core::traits::ProgressSnapshot)>,
    ).expect("Creation failed");

    assert_eq!(comp_summary.files_added, 3);
    assert!(archive_path.exists());

    // 2. Inspect Reader
    let mut reader = open_reader(&archive_path, None).expect("Open failed");
    assert_eq!(reader.format(), ArchiveFormat::Zip);
    assert!(!reader.is_solid());
    let entries = reader.list_entries().expect("List entries failed");
    assert_eq!(entries.len(), 3);

    // 3. Test Integrity Check
    let test_summary = OptiExtractor::test_archive(&archive_path, None, CancellationToken::new())
        .expect("Test failed");
    assert_eq!(test_summary.files_extracted, 3);
    assert!(test_summary.failed_entries.is_empty());

    // 4. Extract All (Parallel)
    let out_dir = dir.path().join("extracted");
    let options = ExtractionOptions {
        output_dir: out_dir.clone(),
        overwrite_policy: OverwritePolicy::Overwrite,
        profile: CompressionProfile::Performance,
        selected_indices: None,
        preserve_attributes: true,
        preserve_timestamps: true,
        password: None,
        keep_broken: false,
        strip_prefix: None,
    };

    let ext_summary = OptiExtractor::extract(
        &archive_path,
        options,
        CancellationToken::new(),
        None::<fn(&optirar_core::traits::ProgressSnapshot)>,
    ).expect("Extraction failed");

    assert_eq!(ext_summary.files_extracted, 3);

    // 5. Verify Content
    let read1 = fs::read_to_string(out_dir.join("file1.txt")).expect("Read file1 failed");
    let read2 = fs::read_to_string(out_dir.join("file2.txt")).expect("Read file2 failed");
    let read3 = fs::read_to_string(out_dir.join("file3.txt")).expect("Read file3 failed");

    assert_eq!(read1, "First test file content for OptiRAR");
    assert_eq!(read2, "Second test file content with longer text data for deflate compression testing");
    assert_eq!(read3, "Nested subfolder file content");
}

#[test]
fn test_tar_roundtrip_extraction() {
    let dir = tempdir().unwrap();
    let src_dir = dir.path().join("src_tar");
    fs::create_dir(&src_dir).unwrap();

    let f1 = src_dir.join("alpha.txt");
    let f2 = src_dir.join("beta.txt");
    fs::write(&f1, "Alpha payload").unwrap();
    fs::write(&f2, "Beta payload in solid tar archive").unwrap();

    let archive_path = dir.path().join("archive.tar");
    let sources = vec![f1, f2];

    // 1. Create TAR
    let comp_summary = OptiCompressor::create_archive(
        &archive_path,
        &sources,
        ArchiveFormat::Tar,
        CompressionLevel::Normal,
        CancellationToken::new(),
        None::<fn(&optirar_core::traits::ProgressSnapshot)>,
    ).expect("TAR creation failed");

    assert_eq!(comp_summary.files_added, 2);

    // 2. Verify Reader format & solid flag
    let reader = open_reader(&archive_path, None).unwrap();
    assert_eq!(reader.format(), ArchiveFormat::Tar);
    assert!(reader.is_solid());

    // 3. Extract TAR (Streaming solid pipeline)
    let out_dir = dir.path().join("tar_out");
    let options = ExtractionOptions {
        output_dir: out_dir.clone(),
        overwrite_policy: OverwritePolicy::Overwrite,
        profile: CompressionProfile::Balanced,
        selected_indices: None,
        preserve_attributes: true,
        preserve_timestamps: true,
        password: None,
        keep_broken: false,
        strip_prefix: None,
    };

    let summary = OptiExtractor::extract(
        &archive_path,
        options,
        CancellationToken::new(),
        None::<fn(&optirar_core::traits::ProgressSnapshot)>,
    ).expect("TAR extraction failed");

    assert_eq!(summary.files_extracted, 2);
    assert_eq!(fs::read_to_string(out_dir.join("alpha.txt")).unwrap(), "Alpha payload");
    assert_eq!(fs::read_to_string(out_dir.join("beta.txt")).unwrap(), "Beta payload in solid tar archive");
}

#[test]
fn test_cancellation_token() {
    let dir = tempdir().unwrap();
    let src = dir.path().join("data.txt");
    fs::write(&src, "Cancellation test data").unwrap();

    let archive_path = dir.path().join("cancelled.zip");
    OptiCompressor::create_archive(
        &archive_path,
        &[src],
        ArchiveFormat::Zip,
        CompressionLevel::Store,
        CancellationToken::new(),
        None::<fn(&optirar_core::traits::ProgressSnapshot)>,
    ).unwrap();

    let cancel = CancellationToken::new();
    cancel.cancel(); // Pre-cancelled

    let options = ExtractionOptions {
        output_dir: dir.path().join("out"),
        overwrite_policy: OverwritePolicy::Overwrite,
        profile: CompressionProfile::Balanced,
        selected_indices: None,
        preserve_attributes: true,
        preserve_timestamps: true,
        password: None,
        keep_broken: false,
        strip_prefix: None,
    };

    let res = OptiExtractor::extract(
        &archive_path,
        options,
        cancel,
        None::<fn(&optirar_core::traits::ProgressSnapshot)>,
    );

    assert!(res.is_err());
}
