//! Carrying out a plan, safely.
//!
//! [`crate::planning`] decides *what* should be renamed; this module decides *how*
//! those renames run, so the safety rules live in exactly one place for every
//! front-end.
//!
//! That safety rests on a two-phase model: the state of every source and
//! destination is recorded when a plan is prepared, and checked again immediately
//! before the renames run. If anything moved in between, nothing is applied at
//! all — a stale preview can never rename the wrong file.

use std::fmt;
use std::fs;
use std::path::Path;
use std::time::SystemTime;

use crate::paths::{display_path, file_name};
use crate::planning::{RenameOp, RenamePlan};

/// A cheap fingerprint of a path, used to spot changes between two points in time.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileState {
    exists: bool,
    is_file: bool,
    identity: Option<(u64, u64)>,
    size: Option<u64>,
    modified: Option<SystemTime>,
}

impl FileState {
    pub fn capture(path: &Path) -> Self {
        let Ok(metadata) = fs::metadata(path) else {
            return Self {
                exists: false,
                is_file: false,
                identity: None,
                size: None,
                modified: None,
            };
        };
        Self {
            exists: true,
            is_file: metadata.is_file(),
            identity: file_identity(&metadata),
            size: Some(metadata.len()),
            modified: metadata.modified().ok(),
        }
    }
}

/// The `(device, inode)` pair that tells two files apart on Unix.
#[cfg(unix)]
fn file_identity(metadata: &fs::Metadata) -> Option<(u64, u64)> {
    use std::os::unix::fs::MetadataExt;
    Some((metadata.dev(), metadata.ino()))
}

#[cfg(not(unix))]
fn file_identity(_metadata: &fs::Metadata) -> Option<(u64, u64)> {
    None
}

/// A planned rename, plus the filesystem state observed when the plan was made.
#[derive(Clone, Debug)]
pub struct PreparedOperation {
    pub id: usize,
    pub operation: RenameOp,
    source_state: FileState,
    destination_state: FileState,
}

impl PreparedOperation {
    pub fn source(&self) -> &Path {
        &self.operation.source
    }

    pub fn destination(&self) -> &Path {
        &self.operation.destination
    }
}

/// The result of a single rename, with paths already formatted for display.
#[derive(Clone, Debug)]
pub struct ApplyOutcome {
    pub id: usize,
    pub source: String,
    pub target: String,
    pub error: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApplyStatus {
    Completed,
    Partial,
    Failed,
}

#[derive(Clone, Debug, Default)]
pub struct ApplyResult {
    pub applied: Vec<ApplyOutcome>,
    pub failed: Vec<ApplyOutcome>,
}

impl ApplyResult {
    pub fn status(&self) -> ApplyStatus {
        if self.failed.is_empty() {
            ApplyStatus::Completed
        } else if self.applied.is_empty() {
            ApplyStatus::Failed
        } else {
            ApplyStatus::Partial
        }
    }
}

/// Raised instead of renaming when the filesystem drifted away from the plan.
#[derive(Clone, Debug)]
pub struct PlanChanged {
    pub changes: Vec<String>,
}

impl fmt::Display for PlanChanged {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.changes.join("; "))
    }
}

impl std::error::Error for PlanChanged {}

/// Pair every operation in `plan` with a stable id and a snapshot of its paths.
pub fn prepare_operations(plan: &RenamePlan) -> Vec<PreparedOperation> {
    plan.operations
        .iter()
        .enumerate()
        .map(|(index, operation)| PreparedOperation {
            id: index + 1,
            source_state: FileState::capture(&operation.source),
            destination_state: FileState::capture(&operation.destination),
            operation: operation.clone(),
        })
        .collect()
}

/// Report which prepared operations no longer match what is on disk.
pub fn detect_state_changes(operations: &[PreparedOperation]) -> Vec<String> {
    let mut changes = Vec::new();
    for prepared in operations {
        if FileState::capture(prepared.source()) != prepared.source_state {
            changes.push(format!("source changed: {}", file_name(prepared.source())));
        }
        if FileState::capture(prepared.destination()) != prepared.destination_state {
            changes.push(format!(
                "target changed: {}",
                file_name(prepared.destination())
            ));
        }
    }
    changes
}

/// Execute `operations`, reporting each rename as an [`ApplyOutcome`].
///
/// With `verify`, the whole batch is refused when any path drifted since the plan
/// was prepared — there is no partial application. Without `force`, an operation
/// fails rather than overwriting a file that is already there.
pub fn apply_operations(
    operations: &[PreparedOperation],
    root: &Path,
    force: bool,
    verify: bool,
) -> Result<ApplyResult, PlanChanged> {
    apply_operations_reporting(operations, root, force, verify, |_| {})
}

/// [`apply_operations`], calling `progress` with the number finished after each
/// rename.
///
/// The interface draws a real progress bar rather than an indeterminate spinner,
/// and a bar needs a count that arrives while the batch is still running — so the
/// caller passes a sink instead of waiting for the finished [`ApplyResult`].
pub fn apply_operations_reporting(
    operations: &[PreparedOperation],
    root: &Path,
    force: bool,
    verify: bool,
    mut progress: impl FnMut(usize),
) -> Result<ApplyResult, PlanChanged> {
    if verify {
        let changes = detect_state_changes(operations);
        if !changes.is_empty() {
            return Err(PlanChanged { changes });
        }
    }

    let mut result = ApplyResult::default();
    for (index, prepared) in operations.iter().enumerate() {
        let source = prepared.source();
        let destination = prepared.destination();
        let outcome = ApplyOutcome {
            id: prepared.id,
            source: display_path(source, root),
            target: display_path(destination, root),
            error: None,
        };
        match rename(source, destination, force) {
            Ok(()) => result.applied.push(outcome),
            Err(error) => result.failed.push(ApplyOutcome {
                error: Some(error),
                ..outcome
            }),
        }
        progress(index + 1);
    }
    Ok(result)
}

fn rename(source: &Path, destination: &Path, force: bool) -> Result<(), String> {
    if !force && target_is_occupied(source, destination) {
        return Err(format!("target already exists: {}", file_name(destination)));
    }
    if let Some(parent) = destination.parent() {
        if !parent.exists() {
            return Err(format!("target folder is gone: {}", parent.display()));
        }
    }
    fs::rename(source, destination).map_err(|error| error.to_string())
}

/// Whether `destination` already holds a *different* file than `source`.
///
/// A rename that only changes letter case resolves to the same file on a
/// case-insensitive filesystem, and must not be mistaken for an overwrite.
fn target_is_occupied(source: &Path, destination: &Path) -> bool {
    let Ok(destination_metadata) = fs::symlink_metadata(destination) else {
        return false;
    };
    let Ok(source_metadata) = fs::symlink_metadata(source) else {
        return true;
    };
    match (
        file_identity(&destination_metadata),
        file_identity(&source_metadata),
    ) {
        (Some(left), Some(right)) => left != right,
        // Without inode numbers, compare the resolved paths instead.
        _ => fs::canonicalize(destination).ok() != fs::canonicalize(source).ok(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::planning::{plan_directory, PlanOptions};

    fn write(path: &Path, contents: &str) {
        fs::write(path, contents).unwrap();
    }

    #[test]
    fn applies_a_plan_and_reports_every_rename() {
        let temporary = tempfile::tempdir().unwrap();
        let root = temporary.path();
        write(&root.join("Nebula.S01E01.1080p.mkv"), "video");
        write(&root.join("random.name.S01E01.chs.ass"), "subtitle");

        let plan = plan_directory(root, &PlanOptions::default()).unwrap();
        let prepared = prepare_operations(&plan);
        let result = apply_operations(&prepared, &plan.root, false, true).unwrap();

        assert_eq!(result.status(), ApplyStatus::Completed);
        assert_eq!(result.applied.len(), 1);
        assert!(plan.root.join("Nebula.S01E01.1080p.ass").exists());
        assert!(!plan.root.join("random.name.S01E01.chs.ass").exists());
    }

    #[test]
    fn refuses_the_whole_batch_when_a_file_moved_since_the_preview() {
        let temporary = tempfile::tempdir().unwrap();
        let root = temporary.path();
        write(&root.join("Nebula.S01E01.mkv"), "video");
        write(&root.join("random.S01E01.chs.ass"), "subtitle");
        write(&root.join("Nebula.S01E02.mkv"), "video");
        write(&root.join("random.S01E02.chs.ass"), "subtitle");

        let plan = plan_directory(root, &PlanOptions::default()).unwrap();
        let prepared = prepare_operations(&plan);
        assert_eq!(prepared.len(), 2);
        fs::remove_file(root.join("random.S01E01.chs.ass")).unwrap();

        let error = apply_operations(&prepared, &plan.root, false, true).unwrap_err();
        assert!(error
            .changes
            .iter()
            .any(|change| change.starts_with("source changed")));
        // Nothing at all was applied, including the operation that was still valid.
        assert!(root.join("random.S01E02.chs.ass").exists());
    }

    #[test]
    fn skipping_verification_still_refuses_to_overwrite() {
        let temporary = tempfile::tempdir().unwrap();
        let root = temporary.path();
        write(&root.join("Nebula.S01E01.mkv"), "video");
        write(&root.join("random.S01E01.chs.ass"), "subtitle");

        let plan = plan_directory(root, &PlanOptions::default()).unwrap();
        let prepared = prepare_operations(&plan);
        write(&root.join("Nebula.S01E01.ass"), "already here");

        let result = apply_operations(&prepared, &plan.root, false, false).unwrap();
        assert_eq!(result.status(), ApplyStatus::Failed);
        assert!(result.failed[0]
            .error
            .as_ref()
            .unwrap()
            .contains("already exists"));
        assert_eq!(
            fs::read_to_string(root.join("Nebula.S01E01.ass")).unwrap(),
            "already here"
        );
    }

    #[test]
    fn force_overwrites_an_existing_target() {
        let temporary = tempfile::tempdir().unwrap();
        let root = temporary.path();
        write(&root.join("Nebula.S01E01.mkv"), "video");
        write(&root.join("random.S01E01.chs.ass"), "subtitle");

        let plan = plan_directory(root, &PlanOptions::default()).unwrap();
        let prepared = prepare_operations(&plan);
        write(&root.join("Nebula.S01E01.ass"), "already here");

        let result = apply_operations(&prepared, &plan.root, true, false).unwrap();
        assert_eq!(result.status(), ApplyStatus::Completed);
        assert_eq!(
            fs::read_to_string(root.join("Nebula.S01E01.ass")).unwrap(),
            "subtitle"
        );
    }

    #[test]
    fn force_preserves_the_target_when_the_source_is_gone() {
        let temporary = tempfile::tempdir().unwrap();
        let root = temporary.path();
        let source = root.join("random.S01E01.chs.ass");
        let destination = root.join("Nebula.S01E01.ass");
        write(&root.join("Nebula.S01E01.mkv"), "video");
        write(&source, "subtitle");
        write(&destination, "already here");

        let plan = plan_directory(
            root,
            &PlanOptions {
                overwrite_existing: true,
                ..PlanOptions::default()
            },
        )
        .unwrap();
        let prepared = prepare_operations(&plan);
        assert_eq!(prepared.len(), 1);
        fs::remove_file(source).unwrap();

        let result = apply_operations(&prepared, &plan.root, true, false).unwrap();
        assert_eq!(result.status(), ApplyStatus::Failed);
        assert_eq!(fs::read_to_string(destination).unwrap(), "already here");
    }

    #[test]
    fn an_untouched_plan_reports_no_changes() {
        let temporary = tempfile::tempdir().unwrap();
        let root = temporary.path();
        write(&root.join("Nebula.S01E01.mkv"), "video");
        write(&root.join("random.S01E01.chs.ass"), "subtitle");

        let plan = plan_directory(root, &PlanOptions::default()).unwrap();
        let prepared = prepare_operations(&plan);
        assert!(detect_state_changes(&prepared).is_empty());
    }
}
