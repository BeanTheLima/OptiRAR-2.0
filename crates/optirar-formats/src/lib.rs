pub mod detector;
pub mod factory;
pub mod gzip_format;
pub mod rar_format;
pub mod sevenz_format;
pub mod tar_format;
pub mod zip_format;
pub mod zstd_format;

pub use detector::FormatDetector;
pub use factory::{create_writer, create_writer_with_password, open_reader};
pub use gzip_format::{GzipReader, GzipWriterWrapper};
pub use rar_format::RarReader;
pub use sevenz_format::{SevenZipReader, SevenZipWriterWrapper};
pub use tar_format::{TarReader, TarWriterWrapper};
pub use zip_format::{modify_zip, CustomZipWriter, ZipAddSource, ZipReader};
pub use zstd_format::{ZstdReader, ZstdWriterWrapper};
