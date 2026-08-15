//! Small path helpers shared by the planner, the CLI and the TUI.

use std::path::{Path, PathBuf};

/// Expand a leading `~` using `$HOME`, leaving everything else untouched.
pub fn expand_user(path: &Path) -> PathBuf {
    let text = path.to_string_lossy();
    let Some(rest) = text.strip_prefix('~') else {
        return path.to_path_buf();
    };
    let Some(home) = std::env::var_os("HOME").map(PathBuf::from) else {
        return path.to_path_buf();
    };
    if rest.is_empty() {
        return home;
    }
    match rest.strip_prefix('/') {
        Some(relative) => home.join(relative),
        // `~other` is another user's home directory, which we do not guess at.
        None => path.to_path_buf(),
    }
}

/// Turn user input into an absolute path, following symlinks where possible.
///
/// A path that does not exist yet still comes back absolute rather than failing,
/// so callers can report it in an error message as the user will recognise it.
pub fn resolve(path: &Path) -> PathBuf {
    let expanded = expand_user(path);
    std::fs::canonicalize(&expanded)
        .or_else(|_| std::path::absolute(&expanded))
        .unwrap_or(expanded)
}

/// Render `path` relative to `root`, falling back to the absolute path.
pub fn display_path(path: &Path, root: &Path) -> String {
    path.strip_prefix(root)
        .map(|relative| relative.to_string_lossy().into_owned())
        .unwrap_or_else(|_| path.to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expands_a_leading_tilde() {
        let home = PathBuf::from(std::env::var_os("HOME").unwrap());
        assert_eq!(expand_user(Path::new("~")), home);
        assert_eq!(expand_user(Path::new("~/videos")), home.join("videos"));
        assert_eq!(expand_user(Path::new("/tmp/~")), PathBuf::from("/tmp/~"));
    }

    #[test]
    fn shows_paths_relative_to_the_scanned_root() {
        let root = Path::new("/library");
        assert_eq!(display_path(Path::new("/library/a/b.srt"), root), "a/b.srt");
        assert_eq!(
            display_path(Path::new("/elsewhere/b.srt"), root),
            "/elsewhere/b.srt"
        );
    }
}
