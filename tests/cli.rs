//! End-to-end tests of the command line, run against the real binary.

use std::fs;
use std::path::Path;
use std::process::{Command, Output};

fn library() -> tempfile::TempDir {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path();
    fs::write(root.join("Nebula.Archive.S01E01.1080p.mkv"), b"video").unwrap();
    fs::write(
        root.join("[Group] Nebula Archive - S01E01.chs.ass"),
        b"subtitle",
    )
    .unwrap();
    fs::write(root.join("Unrelated.Bonus.srt"), b"subtitle").unwrap();
    temporary
}

fn run(root: &Path, arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_submv"))
        .arg(root)
        .args(arguments)
        .output()
        .unwrap()
}

#[test]
fn a_dry_run_prints_the_plan_and_writes_nothing() {
    let temporary = library();
    let output = run(temporary.path(), &["--dry-run"]);
    let text = String::from_utf8(output.stdout).unwrap();

    assert!(output.status.success(), "{text}");
    assert!(text.contains("Directory: ."), "{text}");
    assert!(
        text.contains(
            "[Group] Nebula Archive - S01E01.chs.ass  ->  Nebula.Archive.S01E01.1080p.ass"
        ),
        "{text}"
    );
    assert!(
        text.contains("Unrelated.Bonus.srt: No matching video"),
        "{text}"
    );
    assert!(temporary
        .path()
        .join("[Group] Nebula Archive - S01E01.chs.ass")
        .exists());
}

#[test]
fn no_flag_at_all_still_means_a_dry_run() {
    let temporary = library();
    let output = run(temporary.path(), &[]);
    assert!(output.status.success());
    assert!(temporary
        .path()
        .join("[Group] Nebula Archive - S01E01.chs.ass")
        .exists());
}

#[test]
fn apply_renames_the_files() {
    let temporary = library();
    let output = run(temporary.path(), &["--apply", "--yes"]);
    let text = String::from_utf8(output.stdout).unwrap();

    assert!(output.status.success(), "{text}");
    assert!(text.contains("Renamed 1 of 1."), "{text}");
    assert!(temporary
        .path()
        .join("Nebula.Archive.S01E01.1080p.ass")
        .exists());
    assert!(!temporary
        .path()
        .join("[Group] Nebula Archive - S01E01.chs.ass")
        .exists());
}

#[test]
fn a_cautious_level_matches_less_than_a_relaxed_one() {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path();
    fs::write(root.join("Deep Field Report 2031.mkv"), b"video").unwrap();
    fs::write(root.join("Deep Feild Raport.srt"), b"subtitle").unwrap();

    let relaxed = String::from_utf8(run(root, &["--level", "relaxed"]).stdout).unwrap();
    assert!(relaxed.contains("->"), "{relaxed}");

    let cautious = String::from_utf8(run(root, &["--level", "cautious"]).stdout).unwrap();
    assert!(cautious.contains("No renames planned."), "{cautious}");
}

#[test]
fn refuses_to_overwrite_without_force() {
    let temporary = library();
    let root = temporary.path();
    fs::write(
        root.join("Nebula.Archive.S01E01.1080p.ass"),
        b"already here",
    )
    .unwrap();

    // The plan now aims at a name that already exists, so it picks a free one.
    let text = String::from_utf8(run(root, &["--apply", "--yes"]).stdout).unwrap();
    assert!(
        text.contains("Nebula.Archive.S01E01.1080p.chs.ass"),
        "{text}"
    );
    assert_eq!(
        fs::read_to_string(root.join("Nebula.Archive.S01E01.1080p.ass")).unwrap(),
        "already here"
    );
}

#[test]
fn force_overwrites_the_plain_target() {
    let temporary = library();
    let root = temporary.path();
    let target = root.join("Nebula.Archive.S01E01.1080p.ass");
    fs::write(&target, b"already here").unwrap();

    let output = run(root, &["--apply", "--yes", "--force"]);
    let stdout = String::from_utf8(output.stdout).unwrap();
    let stderr = String::from_utf8(output.stderr).unwrap();

    assert!(output.status.success(), "{stdout}\n{stderr}");
    assert!(
        stdout.contains("->  Nebula.Archive.S01E01.1080p.ass"),
        "{stdout}"
    );
    assert_eq!(fs::read_to_string(target).unwrap(), "subtitle");
    assert!(!root
        .join("[Group] Nebula Archive - S01E01.chs.ass")
        .exists());
}

#[test]
fn strict_and_force_are_mutually_exclusive() {
    let temporary = library();
    let root = temporary.path();
    let source = root.join("[Group] Nebula Archive - S01E01.chs.ass");

    let output = run(root, &["--apply", "--yes", "--strict", "--force"]);
    let stderr = String::from_utf8(output.stderr).unwrap();

    assert_eq!(output.status.code(), Some(2), "{stderr}");
    assert!(source.exists());
    assert!(!root.join("Nebula.Archive.S01E01.1080p.ass").exists());
}

#[test]
fn rejects_a_path_that_is_not_a_directory() {
    let temporary = library();
    let output = run(&temporary.path().join("Unrelated.Bonus.srt"), &[]);
    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8(output.stderr)
        .unwrap()
        .contains("not a directory"));
}

#[test]
fn rejects_dry_run_and_apply_together() {
    let temporary = library();
    let output = run(temporary.path(), &["--dry-run", "--apply"]);
    assert_eq!(output.status.code(), Some(2));
}
