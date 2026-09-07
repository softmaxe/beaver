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
    sanitize(
        path.strip_prefix(root)
            .map(|relative| relative.to_string_lossy().into_owned())
            .unwrap_or_else(|_| path.to_string_lossy().into_owned()),
    )
}

/// The file name as a `String`, falling back to the whole path when there is
/// none (a root, or a path ending in `..`).
pub fn file_name(path: &Path) -> String {
    sanitize(
        path.file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.to_string_lossy().into_owned()),
    )
}

/// A whole path, ready to be shown to a person.
pub fn display(path: &Path) -> String {
    sanitize(path.display().to_string())
}

/// Replace anything in `text` that could rewrite a terminal with `U+FFFD`.
///
/// A filename is untrusted input: whoever produced the file chose it, and a name
/// may hold any byte but `/` and NUL. The CLI prints names straight to a terminal
/// that acts on escape sequences, so a name carrying `ESC[2K\r` can erase the
/// line it was just printed on and redraw a different rename above the "Apply N
/// renames?" prompt — the user would then confirm something other than what is
/// about to happen. Bidirectional overrides do the same by reordering what is
/// displayed. Both are replaced with a visible marker rather than dropped, so a
/// doctored name reads as suspicious instead of merely odd.
///
/// The interface is not affected — ratatui writes cells, not escape sequences —
/// but it renders the same names, and one rule for both means neither front-end
/// can be the one that forgot.
pub fn sanitize(text: String) -> String {
    if !text.chars().any(is_display_hazard) {
        return text;
    }
    text.chars()
        .map(|character| {
            if is_display_hazard(character) {
                char::REPLACEMENT_CHARACTER
            } else {
                character
            }
        })
        .collect()
}

fn is_display_hazard(character: char) -> bool {
    character.is_control()
        || matches!(character,
            // Bidirectional controls, which reorder what is displayed.
            '\u{061c}' | '\u{200e}' | '\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}'
            // Line and paragraph separators, and the zero-width no-break space.
            | '\u{2028}' | '\u{2029}' | '\u{feff}')
}

/// Sort key that keeps output stable regardless of directory iteration order.
pub fn sort_key(path: &Path) -> String {
    path.to_string_lossy().to_lowercase()
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
    fn strips_escape_sequences_and_bidi_controls_from_names() {
        // A name that would otherwise erase the line it was printed on.
        assert_eq!(
            file_name(Path::new("/library/evil\u{1b}[2K\rHarmless.srt")),
            "evil\u{fffd}[2K\u{fffd}Harmless.srt"
        );
        assert_eq!(
            display_path(
                Path::new("/library/\u{202e}tpircs.srt"),
                Path::new("/library")
            ),
            "\u{fffd}tpircs.srt"
        );
        assert_eq!(
            file_name(Path::new("/library/ordinary.srt")),
            "ordinary.srt"
        );
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
