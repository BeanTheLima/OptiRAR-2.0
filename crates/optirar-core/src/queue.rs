use crate::types::{ArchiveFormat, CompressionProfile};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};

static NEXT_TASK_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TaskKind {
    Extract {
        archive: PathBuf,
        destination: PathBuf,
        smart: bool,
    },
    Create {
        output: PathBuf,
        sources: Vec<PathBuf>,
        format: ArchiveFormat,
        profile: CompressionProfile,
    },
    Test {
        archive: PathBuf,
    },
    Hash {
        file: PathBuf,
    },
    Compare {
        archive_a: PathBuf,
        archive_b: PathBuf,
    },
}

impl TaskKind {
    pub fn description(&self) -> String {
        match self {
            TaskKind::Extract { archive, .. } => {
                format!("Extract {}", archive.file_name().and_then(|n| n.to_str()).unwrap_or("archive"))
            }
            TaskKind::Create { output, .. } => {
                format!("Create {}", output.file_name().and_then(|n| n.to_str()).unwrap_or("archive"))
            }
            TaskKind::Test { archive } => {
                format!("Test {}", archive.file_name().and_then(|n| n.to_str()).unwrap_or("archive"))
            }
            TaskKind::Hash { file } => {
                format!("Compute Hashes for {}", file.file_name().and_then(|n| n.to_str()).unwrap_or("file"))
            }
            TaskKind::Compare { archive_a, archive_b } => {
                format!(
                    "Compare {} & {}",
                    archive_a.file_name().and_then(|n| n.to_str()).unwrap_or("A"),
                    archive_b.file_name().and_then(|n| n.to_str()).unwrap_or("B")
                )
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TaskState {
    Pending,
    Running {
        progress: f32, // 0.0 to 100.0
        current_file: String,
        throughput_mb_s: f64,
    },
    Completed {
        message: String,
        elapsed_secs: f64,
    },
    Failed {
        error: String,
    },
    Cancelled,
}

impl TaskState {
    pub fn is_finished(&self) -> bool {
        matches!(
            self,
            TaskState::Completed { .. } | TaskState::Failed { .. } | TaskState::Cancelled
        )
    }

    pub fn is_running(&self) -> bool {
        matches!(self, TaskState::Running { .. })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueItem {
    pub id: u64,
    pub title: String,
    pub kind: TaskKind,
    pub state: TaskState,
}

impl QueueItem {
    pub fn new(kind: TaskKind) -> Self {
        let id = NEXT_TASK_ID.fetch_add(1, Ordering::SeqCst);
        let title = kind.description();
        Self {
            id,
            title,
            kind,
            state: TaskState::Pending,
        }
    }
}

/// Shared Task Queue for managing background extraction/compression jobs.
#[derive(Debug, Clone, Default)]
pub struct TaskQueue {
    items: Arc<RwLock<Vec<QueueItem>>>,
}

impl TaskQueue {
    pub fn new() -> Self {
        Self {
            items: Arc::new(RwLock::new(Vec::new())),
        }
    }

    pub fn enqueue(&self, kind: TaskKind) -> u64 {
        let item = QueueItem::new(kind);
        let id = item.id;
        if let Ok(mut items) = self.items.write() {
            items.push(item);
        }
        id
    }

    pub fn list(&self) -> Vec<QueueItem> {
        self.items.read().map(|items| items.clone()).unwrap_or_default()
    }

    pub fn get(&self, id: u64) -> Option<QueueItem> {
        self.items
            .read()
            .ok()?
            .iter()
            .find(|item| item.id == id)
            .cloned()
    }

    pub fn update_state(&self, id: u64, state: TaskState) {
        if let Ok(mut items) = self.items.write() {
            if let Some(item) = items.iter_mut().find(|i| i.id == id) {
                item.state = state;
            }
        }
    }

    pub fn cancel(&self, id: u64) {
        self.update_state(id, TaskState::Cancelled);
    }

    pub fn remove(&self, id: u64) {
        if let Ok(mut items) = self.items.write() {
            items.retain(|i| i.id != id);
        }
    }

    pub fn clear_completed(&self) {
        if let Ok(mut items) = self.items.write() {
            items.retain(|i| !i.state.is_finished());
        }
    }

    pub fn has_active_tasks(&self) -> bool {
        self.items
            .read()
            .map(|items| items.iter().any(|i| i.state.is_running() || matches!(i.state, TaskState::Pending)))
            .unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_task_queue_operations() {
        let q = TaskQueue::new();
        let id = q.enqueue(TaskKind::Test {
            archive: PathBuf::from("test.zip"),
        });

        assert_eq!(q.list().len(), 1);
        assert_eq!(q.get(id).unwrap().state, TaskState::Pending);

        q.update_state(
            id,
            TaskState::Running {
                progress: 50.0,
                current_file: "file.txt".to_string(),
                throughput_mb_s: 15.5,
            },
        );

        assert!(q.get(id).unwrap().state.is_running());

        q.update_state(
            id,
            TaskState::Completed {
                message: "OK".into(),
                elapsed_secs: 1.2,
            },
        );

        assert!(q.get(id).unwrap().state.is_finished());
        q.clear_completed();
        assert_eq!(q.list().len(), 0);
    }
}
