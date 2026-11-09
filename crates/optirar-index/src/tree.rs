use crate::catalog::ArchiveCatalog;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// A node in the hierarchical folder tree of an archive.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FolderNode {
    /// Directory name (e.g. "src").
    pub name: String,
    /// Full normalized relative path (e.g. "optirar/src").
    pub full_path: String,
    /// Subfolders sorted by name.
    pub subfolders: BTreeMap<String, FolderNode>,
    /// Indices of files contained directly within this folder.
    pub file_indices: Vec<usize>,
}

impl FolderNode {
    pub fn new(name: String, full_path: String) -> Self {
        Self {
            name,
            full_path,
            subfolders: BTreeMap::new(),
            file_indices: Vec::new(),
        }
    }

    /// Construct a complete hierarchical folder tree from an ArchiveCatalog.
    pub fn build_from_catalog(catalog: &ArchiveCatalog) -> Self {
        let mut root = FolderNode::new(String::new(), String::new());

        for (idx, entry) in catalog.entries.iter().enumerate() {
            let normalized = entry.path.replace('\\', "/");
            let parts: Vec<&str> = normalized.split('/').filter(|p| !p.is_empty()).collect();

            if parts.is_empty() {
                continue;
            }

            if entry.is_directory {
                root.insert_folder(&parts, 0);
            } else {
                root.insert_file(&parts, 0, idx);
            }
        }

        root
    }

    fn insert_folder(&mut self, parts: &[&str], depth: usize) {
        if depth >= parts.len() {
            return;
        }

        let name = parts[depth].to_string();
        let full_path = parts[..=depth].join("/");

        let child = self
            .subfolders
            .entry(name.clone())
            .or_insert_with(|| FolderNode::new(name, full_path));

        child.insert_folder(parts, depth + 1);
    }

    fn insert_file(&mut self, parts: &[&str], depth: usize, entry_index: usize) {
        if depth + 1 >= parts.len() {
            // Leaf file inside this folder
            self.file_indices.push(entry_index);
            return;
        }

        let folder_name = parts[depth].to_string();
        let full_path = parts[..=depth].join("/");

        let child = self
            .subfolders
            .entry(folder_name.clone())
            .or_insert_with(|| FolderNode::new(folder_name, full_path));

        child.insert_file(parts, depth + 1, entry_index);
    }
}
