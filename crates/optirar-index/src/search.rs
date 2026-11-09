use crate::catalog::ArchiveCatalog;

/// Fast search and filtering engine for archive entries.
#[derive(Debug, Clone, Default)]
pub struct SearchFilter {
    pub query: String,
    pub min_size: Option<u64>,
    pub max_size: Option<u64>,
    pub directories_only: bool,
    pub files_only: bool,
}

impl SearchFilter {
    pub fn new(query: &str) -> Self {
        Self {
            query: query.trim().to_lowercase(),
            ..Default::default()
        }
    }

    /// Execute search against the archive catalog and return indices of matching entries.
    pub fn search(&self, catalog: &ArchiveCatalog) -> Vec<usize> {
        let is_wildcard = self.query.contains('*') || self.query.contains('?');

        catalog
            .entries
            .iter()
            .enumerate()
            .filter(|(_, entry)| {
                // Type filters
                if self.directories_only && !entry.is_directory {
                    return false;
                }
                if self.files_only && entry.is_directory {
                    return false;
                }

                // Size filters
                if let Some(min) = self.min_size {
                    if entry.uncompressed_size < min {
                        return false;
                    }
                }
                if let Some(max) = self.max_size {
                    if entry.uncompressed_size > max {
                        return false;
                    }
                }

                // Query filter
                if self.query.is_empty() {
                    return true;
                }

                let lower_path = entry.path.to_ascii_lowercase();
                if is_wildcard {
                    match_wildcard(&self.query, &lower_path)
                } else {
                    lower_path.contains(&self.query)
                }
            })
            .map(|(idx, _)| idx)
            .collect()
    }
}

/// Simple glob wildcard matcher (* and ?).
fn match_wildcard(pattern: &str, text: &str) -> bool {
    let p_chars: Vec<char> = pattern.chars().collect();
    let t_chars: Vec<char> = text.chars().collect();

    let mut p_idx = 0;
    let mut t_idx = 0;
    let mut star_idx = None;
    let mut match_idx = 0;

    while t_idx < t_chars.len() {
        if p_idx < p_chars.len() && (p_chars[p_idx] == '?' || p_chars[p_idx] == t_chars[t_idx]) {
            p_idx += 1;
            t_idx += 1;
        } else if p_idx < p_chars.len() && p_chars[p_idx] == '*' {
            star_idx = Some(p_idx);
            match_idx = t_idx;
            p_idx += 1;
        } else if let Some(star) = star_idx {
            p_idx = star + 1;
            match_idx += 1;
            t_idx = match_idx;
        } else {
            return false;
        }
    }

    while p_idx < p_chars.len() && p_chars[p_idx] == '*' {
        p_idx += 1;
    }

    p_idx == p_chars.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wildcard_matching() {
        assert!(match_wildcard("*.rs", "main.rs"));
        assert!(match_wildcard("folder/*.txt", "folder/document.txt"));
        assert!(!match_wildcard("*.png", "document.pdf"));
    }
}
