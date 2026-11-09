use crate::detector::FormatDetector;
use crate::gzip_format::{GzipReader, GzipWriterWrapper};
use crate::rar_format::RarReader;
use crate::sevenz_format::{SevenZipReader, SevenZipWriterWrapper};
use crate::tar_format::{TarReader, TarWriterWrapper};
use crate::zip_format::{CustomZipWriter, ZipReader};
use crate::zstd_format::{ZstdReader, ZstdWriterWrapper};
use optirar_core::error::{OptiError, Result};
use optirar_core::traits::{ArchiveReader, ArchiveWriter};
use optirar_core::types::{ArchiveFormat, CompressionLevel};
use std::path::Path;

/// Factory for opening any supported archive format for reading and extraction.
pub fn open_reader<P: AsRef<Path>>(
    path: P,
    password: Option<&str>,
) -> Result<Box<dyn ArchiveReader>> {
    let path_ref = path.as_ref();
    let format = FormatDetector::detect(path_ref)?;

    match format {
        ArchiveFormat::Zip => Ok(Box::new(ZipReader::open_with_password(path_ref, password)?)),
        ArchiveFormat::Tar => Ok(Box::new(TarReader::open(path_ref)?)),
        ArchiveFormat::Gzip => Ok(Box::new(GzipReader::open(path_ref)?)),
        ArchiveFormat::Zstd => Ok(Box::new(ZstdReader::open(path_ref)?)),
        ArchiveFormat::SevenZip => Ok(Box::new(SevenZipReader::open(path_ref, password)?)),
        ArchiveFormat::Rar4 | ArchiveFormat::Rar5 => Ok(Box::new(RarReader::open(path_ref)?)),
        ArchiveFormat::Unknown => Err(OptiError::FormatUnsupported(ArchiveFormat::Unknown)),
    }
}

/// Factory for creating new archives in supported open formats.
pub fn create_writer<P: AsRef<Path>>(
    path: P,
    format: ArchiveFormat,
    level: CompressionLevel,
) -> Result<Box<dyn ArchiveWriter>> {
    create_writer_with_password(path, format, level, None)
}

/// Like [`create_writer`]; ZIP (AES-256) and 7z encrypt contents when a password is given.
pub fn create_writer_with_password<P: AsRef<Path>>(
    path: P,
    format: ArchiveFormat,
    level: CompressionLevel,
    password: Option<&str>,
) -> Result<Box<dyn ArchiveWriter>> {
    let path_ref = path.as_ref();
    let password = password.filter(|p| !p.is_empty());
    if password.is_some() && !matches!(format, ArchiveFormat::Zip | ArchiveFormat::SevenZip) {
        return Err(OptiError::Internal(format!("{} does not support passwords", format.name())));
    }
    match format {
        ArchiveFormat::Zip => Ok(Box::new(CustomZipWriter::create_with_password(path_ref, level, password)?)),
        ArchiveFormat::Tar => Ok(Box::new(TarWriterWrapper::create(path_ref)?)),
        ArchiveFormat::Gzip => Ok(Box::new(GzipWriterWrapper::create(path_ref, level)?)),
        ArchiveFormat::Zstd => Ok(Box::new(ZstdWriterWrapper::create(path_ref, level)?)),
        ArchiveFormat::SevenZip => Ok(Box::new(SevenZipWriterWrapper::create(path_ref, level, password)?)),
        ArchiveFormat::Rar4 | ArchiveFormat::Rar5 => Err(OptiError::CreationNotSupported(format)),
        ArchiveFormat::Unknown => Err(OptiError::FormatUnsupported(ArchiveFormat::Unknown)),
    }
}
