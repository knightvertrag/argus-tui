use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// In-memory source files keyed by path, loaded on demand from disk.
#[derive(Debug, Default)]
pub struct SourceCache {
    files: HashMap<PathBuf, CachedFile>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CachedFile {
    pub path: PathBuf,
    pub lines: Vec<String>,
}

impl SourceCache {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn load(&mut self, path: &Path) -> std::io::Result<&CachedFile> {
        if !self.files.contains_key(path) {
            let text = std::fs::read_to_string(path)?;
            let lines: Vec<String> = text.lines().map(str::to_string).collect();
            tracing::debug!(path = %path.display(), lines = lines.len(), "loaded source");
            self.files.insert(
                path.to_path_buf(),
                CachedFile {
                    path: path.to_path_buf(),
                    lines,
                },
            );
        }
        Ok(self.files.get(path).expect("just inserted"))
    }

    /// 1-based line, matching DAP. `None` if the line is past EOF.
    pub fn line(&mut self, path: &Path, line: i64) -> std::io::Result<Option<&str>> {
        let file = self.load(path)?;
        if line < 1 {
            tracing::debug!(path = %path.display(), line, "source line out of range");
            return Ok(None);
        }
        match file.lines.get((line as usize) - 1).map(String::as_str) {
            Some(text) => Ok(Some(text)),
            None => {
                tracing::debug!(path = %path.display(), line, "source line out of range");
                Ok(None)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn loads_one_based_lines() {
        let dir = std::env::temp_dir();
        let path = dir.join("argus-source-cache-test.c");
        let mut file = std::fs::File::create(&path).unwrap();
        writeln!(file, "int main(void) {{").unwrap();
        writeln!(file, "    return 0;").unwrap();
        writeln!(file, "}}").unwrap();
        drop(file);

        let mut cache = SourceCache::new();
        assert_eq!(cache.line(&path, 1).unwrap(), Some("int main(void) {"));
        assert_eq!(cache.line(&path, 2).unwrap(), Some("    return 0;"));
        assert_eq!(cache.line(&path, 4).unwrap(), None);
        let _ = std::fs::remove_file(path);
    }
}
