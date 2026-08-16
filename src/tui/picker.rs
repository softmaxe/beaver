//! A small directory browser, so a path can be found without typing it.
//!
//! Only directories are listed: the point is to choose a folder to scan, and
//! showing the media files inside it would just be noise.

use std::fs;
use std::path::{Path, PathBuf};

use ratatui::widgets::ListState;

use crate::tui::app::move_selection;

pub struct Picker {
    pub current: PathBuf,
    pub entries: Vec<PathBuf>,
    pub state: ListState,
    /// Set when a directory could not be read, shown in place of its contents.
    pub error: Option<String>,
}

impl Picker {
    pub fn new(start: &Path) -> Self {
        let mut picker = Self {
            current: start.to_path_buf(),
            entries: Vec::new(),
            state: ListState::default(),
            error: None,
        };
        picker.reload();
        picker
    }

    fn reload(&mut self) {
        self.entries.clear();
        self.error = None;
        match fs::read_dir(&self.current) {
            Ok(entries) => {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if !path.is_dir() || is_hidden(&path) {
                        continue;
                    }
                    self.entries.push(path);
                }
                self.entries
                    .sort_by_key(|path| path.to_string_lossy().to_lowercase());
            }
            Err(error) => self.error = Some(error.to_string()),
        }
        self.state.select(if self.entries.is_empty() {
            None
        } else {
            Some(0)
        });
    }

    /// Move the highlight by `delta` rows, staying inside the listing.
    pub fn move_by(&mut self, delta: isize) {
        let selected = move_selection(self.state.selected(), self.entries.len(), delta);
        self.state.select(selected);
    }

    /// Descend into the highlighted directory.
    pub fn enter(&mut self) {
        let Some(index) = self.state.selected() else {
            return;
        };
        let Some(path) = self.entries.get(index).cloned() else {
            return;
        };
        self.current = path;
        self.reload();
    }

    /// Go back to the parent directory, keeping the one just left highlighted.
    pub fn leave(&mut self) {
        let Some(parent) = self.current.parent().map(Path::to_path_buf) else {
            return;
        };
        let previous = std::mem::replace(&mut self.current, parent);
        self.reload();
        if let Some(index) = self.entries.iter().position(|entry| *entry == previous) {
            self.state.select(Some(index));
        }
    }
}

fn is_hidden(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.starts_with('.'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_directories_and_walks_up_and_down() {
        let temporary = tempfile::tempdir().unwrap();
        let root = temporary.path();
        fs::create_dir(root.join("season one")).unwrap();
        fs::create_dir(root.join("season two")).unwrap();
        fs::create_dir(root.join(".hidden")).unwrap();
        fs::write(root.join("a-file.mkv"), b"").unwrap();

        let mut picker = Picker::new(root);
        assert_eq!(picker.entries.len(), 2);
        assert_eq!(picker.state.selected(), Some(0));

        picker.move_by(1);
        picker.enter();
        assert_eq!(picker.current, root.join("season two"));
        assert!(picker.entries.is_empty());

        picker.leave();
        assert_eq!(picker.current, root);
        assert_eq!(picker.state.selected(), Some(1));
    }
}
