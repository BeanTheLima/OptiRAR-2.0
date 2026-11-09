pub mod compare;
pub mod compressor;
pub mod duplicates;
pub mod extractor;
pub mod preview;
pub mod progress;
pub mod smart_extract;

pub use compare::{ArchiveComparator, ArchiveDiffResult};
pub use compressor::{CompressionSummary, OptiCompressor};
pub use duplicates::{DuplicateFinder, DuplicateGroup, DuplicateReport};
pub use extractor::{ExtractionSummary, OptiExtractor};
pub use preview::{ArchivePreviewer, PreviewContent};
pub use progress::ProgressTracker;
pub use smart_extract::compute_smart_destination;
