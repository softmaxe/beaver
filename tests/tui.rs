//! End-to-end tests of the terminal interface, driven by keystrokes and read
//! back from a rendered frame — the same path a person takes.

use std::fs;
use std::path::Path;
use std::time::{Duration, Instant};

use ratatui::backend::TestBackend;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::Terminal;

use submv::presentation::MatchLevel;
use submv::tui::app::{App, Focus, Tab};
use submv::tui::ui;

/// Drive the app to a settled state and return what the screen shows.
struct Harness {
    app: App,
    terminal: Terminal<TestBackend>,
}

impl Harness {
    fn new(directory: &Path) -> Self {
        Self {
            app: App::new().with_directory(directory),
            terminal: Terminal::new(TestBackend::new(120, 32)).unwrap(),
        }
    }

    fn press(&mut self, code: KeyCode) {
        self.app.handle_key(KeyEvent::new(code, KeyModifiers::NONE));
    }

    fn press_control(&mut self, code: KeyCode) {
        self.app
            .handle_key(KeyEvent::new(code, KeyModifiers::CONTROL));
    }

    /// Wait for a worker thread to report back, then redraw.
    fn settle(&mut self, condition: impl Fn(&App) -> bool) {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            self.app.poll_workers();
            if condition(&self.app) {
                break;
            }
            assert!(Instant::now() < deadline, "the app never settled");
            std::thread::sleep(Duration::from_millis(5));
        }
        self.draw();
    }

    fn draw(&mut self) {
        let app = &mut self.app;
        self.terminal.draw(|frame| ui::draw(frame, app)).unwrap();
    }

    /// The bottom row on its own: the hint line, which changes with the focus.
    fn footer(&self) -> String {
        self.screen().lines().last().unwrap_or_default().to_string()
    }

    /// The whole screen as text, one line per row.
    fn screen(&self) -> String {
        let buffer = self.terminal.backend().buffer();
        (0..buffer.area.height)
            .map(|row| {
                (0..buffer.area.width)
                    .map(|column| buffer[(column, row)].symbol())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}

fn library() -> tempfile::TempDir {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path();
    fs::write(root.join("Nebula.Archive.S01E01.1080p.mkv"), b"video").unwrap();
    fs::write(root.join("Nebula.Archive.S01E02.1080p.mkv"), b"video").unwrap();
    fs::write(
        root.join("[Group] Nebula Archive - S01E01.chs.ass"),
        b"subtitle",
    )
    .unwrap();
    fs::write(
        root.join("[Group] Nebula Archive - S01E02.chs.ass"),
        b"subtitle",
    )
    .unwrap();
    fs::write(root.join("Unrelated.Bonus.srt"), b"subtitle").unwrap();
    temporary
}

/// A misspelt pair that only the looser match levels accept: the two stems score
/// 0.76, which sits above "balanced" but below "cautious".
fn fuzzy_library() -> tempfile::TempDir {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path();
    fs::write(root.join("Deep Field Report 2031.mkv"), b"video").unwrap();
    fs::write(root.join("Deep Feild Raport.srt"), b"subtitle").unwrap();
    temporary
}

/// Fourteen matched pairs: more than a full page, so the page keys have room to
/// move and something to clamp against.
fn long_library() -> tempfile::TempDir {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path();
    for episode in 1..=14 {
        fs::write(root.join(format!("Harbour.S01E{episode:02}.mkv")), b"video").unwrap();
        fs::write(
            root.join(format!("[G] Harbour - S01E{episode:02}.chs.ass")),
            b"subtitle",
        )
        .unwrap();
    }
    temporary
}

/// Three sibling folders, one of them with a folder of its own, so the picker has
/// somewhere to move up and down and somewhere to descend into.
fn picker_library() -> tempfile::TempDir {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path();
    fs::create_dir_all(root.join("season 1/extras")).unwrap();
    fs::create_dir(root.join("season 2")).unwrap();
    fs::create_dir(root.join("season 3")).unwrap();
    temporary
}

/// A library with a second season tucked into a subfolder.
fn nested_library() -> tempfile::TempDir {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path();
    fs::write(root.join("Harbour.S02E01.mkv"), b"video").unwrap();
    fs::write(root.join("[G] Harbour - S02E01.ass"), b"subtitle").unwrap();
    fs::create_dir(root.join("season 3")).unwrap();
    fs::write(root.join("season 3/Harbour.S03E01.mkv"), b"video").unwrap();
    fs::write(root.join("season 3/[G] Harbour - S03E01.ass"), b"subtitle").unwrap();
    temporary
}

/// A preview that describes the disk as it is right now.
fn fresh_preview(app: &App) -> bool {
    !app.scanning && app.preview.as_ref().is_some_and(|preview| !preview.stale)
}

#[test]
fn previews_ticks_and_applies() {
    let temporary = library();
    let mut harness = Harness::new(temporary.path());

    harness.press(KeyCode::Enter);
    harness.settle(|app| app.preview.is_some());

    let screen = harness.screen();
    assert!(screen.contains("To rename (2)"), "{screen}");
    assert!(screen.contains("Skipped (1)"), "{screen}");
    assert!(screen.contains("episode S01E01"), "{screen}");
    assert!(screen.contains("2 of 2 ticked"), "{screen}");
    assert!(screen.contains("Preview ready"), "{screen}");
    // A successful preview hands the keyboard to the results, so the workflow
    // shortcuts work without pressing anything else first.
    assert_eq!(harness.app.focus, Focus::Results);
    assert!(harness.app.can_apply());

    // Untick the highlighted rename; only the other one should be applied.
    harness.press(KeyCode::Char(' '));
    harness.draw();
    assert!(harness.screen().contains("1 of 2 ticked"));

    harness.press(KeyCode::Char('a'));
    harness.draw();
    assert!(harness.screen().contains("Confirm apply"));
    harness.press(KeyCode::Enter);
    harness.settle(|app| !app.applying && app.status.text.starts_with("Renamed"));

    let root = temporary.path();
    assert!(root.join("Nebula.Archive.S01E02.1080p.ass").exists());
    assert!(root
        .join("[Group] Nebula Archive - S01E01.chs.ass")
        .exists());
    assert!(harness.screen().contains("Renamed 1 file"));
    // The files moved, so the preview no longer describes the disk.
    assert!(!harness.app.can_apply());
}

#[test]
fn changing_an_option_invalidates_the_preview() {
    let temporary = library();
    let mut harness = Harness::new(temporary.path());

    harness.press(KeyCode::Enter);
    harness.settle(|app| app.preview.is_some());
    assert!(harness.app.can_apply());

    // Tab round from the results, past the path field, onto the recursive
    // switch, and flip it.
    harness.press(KeyCode::Tab);
    assert_eq!(harness.app.focus, Focus::Directory);
    harness.press(KeyCode::Tab);
    assert_eq!(harness.app.focus, Focus::Recursive);
    harness.press(KeyCode::Char(' '));
    harness.draw();

    assert!(!harness.app.can_apply());
    assert!(harness.screen().contains("Options changed — preview again"));

    // Applying anyway is refused rather than acting on a stale plan.
    harness.press(KeyCode::Char('a'));
    assert!(harness.app.modal.is_none());
}

#[test]
fn a_demo_plan_can_never_be_applied() {
    let temporary = tempfile::tempdir().unwrap();
    let mut harness = Harness::new(temporary.path());

    harness.press(KeyCode::Esc);
    harness.press(KeyCode::Char('d'));
    harness.draw();

    assert!(harness.screen().contains("Demo mode"));
    assert!(harness.screen().contains("To rename (3)"));
    assert!(!harness.app.can_apply());

    harness.press(KeyCode::Char('a'));
    harness.draw();
    assert!(harness.app.modal.is_none());
    assert!(harness.screen().contains("Demo mode never writes to disk"));
}

#[test]
fn bulk_ticking_and_the_skipped_tab() {
    let temporary = library();
    let mut harness = Harness::new(temporary.path());
    harness.press(KeyCode::Enter);
    harness.settle(|app| app.preview.is_some());

    harness.press_control(KeyCode::Char('r'));
    harness.draw();
    assert!(harness.screen().contains("0 of 2 ticked"));
    assert!(!harness.app.can_apply());

    harness.press_control(KeyCode::Char('a'));
    harness.draw();
    assert!(harness.screen().contains("2 of 2 ticked"));

    harness.press(KeyCode::Right);
    harness.draw();
    assert_eq!(harness.app.tab, Tab::Skipped);
    let screen = harness.screen();
    assert!(screen.contains("Unrelated.Bonus.srt"), "{screen}");
    assert!(screen.contains("No matching video"), "{screen}");
}

#[test]
fn reports_a_directory_that_does_not_exist() {
    let mut harness = Harness::new(Path::new("/no/such/folder/here"));
    harness.press(KeyCode::Enter);
    harness.draw();
    assert!(harness.screen().contains("Not a directory"));
    assert!(harness.app.preview.is_none());
}

#[test]
fn the_help_modal_lists_every_shortcut() {
    let temporary = tempfile::tempdir().unwrap();
    let mut harness = Harness::new(temporary.path());
    harness.press(KeyCode::Esc);
    harness.press(KeyCode::Char('?'));
    harness.draw();

    let screen = harness.screen();
    assert!(screen.contains("Keyboard shortcuts"), "{screen}");
    assert!(screen.contains("Tick or untick"), "{screen}");

    harness.press(KeyCode::Esc);
    harness.draw();
    assert!(!harness.screen().contains("Keyboard shortcuts"));
}

#[test]
fn the_layout_survives_a_small_terminal() {
    let temporary = library();
    let mut harness = Harness::new(temporary.path());
    harness.terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();

    harness.press(KeyCode::Enter);
    harness.settle(|app| app.preview.is_some());

    let screen = harness.screen();
    assert!(screen.contains("To rename (2)"), "{screen}");
    assert!(screen.contains("p preview"), "{screen}");
    // Stacked, not side by side: the setup column keeps the full width.
    assert!(
        screen
            .lines()
            .any(|line| line.contains("Include subfolders")),
        "{screen}"
    );
}

#[test]
fn quits_on_q_and_on_control_c() {
    let temporary = tempfile::tempdir().unwrap();

    let mut harness = Harness::new(temporary.path());
    // The path field has focus, so a bare letter types rather than quitting.
    harness.press(KeyCode::Char('q'));
    assert!(!harness.app.should_quit);
    harness.press(KeyCode::Esc);
    harness.press(KeyCode::Char('q'));
    assert!(harness.app.should_quit);

    // Control+C always quits, whatever holds the keyboard.
    let mut harness = Harness::new(temporary.path());
    harness.press_control(KeyCode::Char('c'));
    assert!(harness.app.should_quit);
}

#[test]
fn quit_keys_wait_for_an_apply_to_finish() {
    let temporary = tempfile::tempdir().unwrap();

    let mut harness = Harness::new(temporary.path());
    harness.app.applying = true;
    harness.app.focus = Focus::Results;
    harness.press(KeyCode::Char('q'));
    assert!(!harness.app.should_quit);
    assert_eq!(
        harness.app.status.text,
        "Applying… wait for it to finish before quitting"
    );

    let mut harness = Harness::new(temporary.path());
    harness.app.applying = true;
    harness.press_control(KeyCode::Char('c'));
    assert!(!harness.app.should_quit);
    assert_eq!(
        harness.app.status.text,
        "Applying… wait for it to finish before quitting"
    );
}

#[test]
fn tab_cycles_the_controls_and_esc_jumps_between_the_ends() {
    let temporary = tempfile::tempdir().unwrap();
    let mut harness = Harness::new(temporary.path());

    for expected in [
        Focus::Recursive,
        Focus::Strict,
        Focus::Level,
        Focus::Results,
        Focus::Directory,
    ] {
        harness.press(KeyCode::Tab);
        assert_eq!(harness.app.focus, expected);
    }
    for expected in [
        Focus::Results,
        Focus::Level,
        Focus::Strict,
        Focus::Recursive,
        Focus::Directory,
    ] {
        harness.press(KeyCode::BackTab);
        assert_eq!(harness.app.focus, expected);
    }

    // Up and down step between the setup controls as well.
    harness.press(KeyCode::Tab);
    harness.press(KeyCode::Down);
    assert_eq!(harness.app.focus, Focus::Strict);
    harness.press(KeyCode::Up);
    assert_eq!(harness.app.focus, Focus::Recursive);

    // Esc is the shortcut out of the path field, and back to it.
    harness.app.focus = Focus::Directory;
    harness.press(KeyCode::Esc);
    assert_eq!(harness.app.focus, Focus::Results);
    harness.press(KeyCode::Esc);
    assert_eq!(harness.app.focus, Focus::Directory);
}

#[test]
fn letters_type_into_the_path_field_until_the_focus_leaves_it() {
    let temporary = tempfile::tempdir().unwrap();
    let mut harness = Harness::new(temporary.path());

    // Every one of these is a shortcut elsewhere, and none of them may fire here.
    for character in ['p', 'a', 'd', 'o', '?'] {
        harness.press(KeyCode::Char(character));
    }
    assert!(harness.app.directory.value().ends_with("pado?"));
    assert!(harness.app.modal.is_none());
    assert!(harness.app.preview.is_none());

    harness.press(KeyCode::Esc);
    harness.press(KeyCode::Char('d'));
    harness.draw();
    assert!(harness.screen().contains("Demo mode"));

    harness.press(KeyCode::Char('?'));
    harness.draw();
    assert!(harness.screen().contains("Keyboard shortcuts"));
}

#[test]
fn the_path_can_be_edited_in_place_and_previewed_with_enter() {
    let temporary = library();
    let path = temporary.path().to_string_lossy().into_owned();
    let mut harness = Harness::new(temporary.path());

    // The cursor starts at the end, so backspace trims the path.
    harness.press(KeyCode::Backspace);
    harness.press(KeyCode::Enter);
    harness.draw();
    assert!(harness.screen().contains("Not a directory"));
    assert!(harness.app.preview.is_none());

    harness.press(KeyCode::Char(path.chars().last().unwrap()));
    assert_eq!(harness.app.directory.value(), path);
    harness.press(KeyCode::Enter);
    harness.settle(|app| app.preview.is_some());
    assert!(harness.screen().contains("To rename (2)"));

    // An empty field asks for a path rather than scanning anything.
    let mut harness = Harness::new(Path::new(""));
    harness.press(KeyCode::Enter);
    harness.draw();
    assert!(harness.screen().contains("Enter a directory path first"));
    assert!(harness.app.preview.is_none());
}

#[test]
fn arrows_set_the_switches_and_step_through_the_match_levels() {
    let temporary = tempfile::tempdir().unwrap();
    let mut harness = Harness::new(temporary.path());

    harness.press(KeyCode::Tab);
    harness.press(KeyCode::Right);
    harness.draw();
    assert!(harness.app.recursive);
    assert!(harness.screen().contains("[✓] Include subfolders"));
    harness.press(KeyCode::Left);
    harness.draw();
    assert!(!harness.app.recursive);
    assert!(harness.screen().contains("[ ] Include subfolders"));

    harness.press(KeyCode::Tab);
    harness.press(KeyCode::Right);
    harness.draw();
    assert!(harness.app.strict);
    assert!(harness.screen().contains("[✓] Strict mode"));

    harness.press(KeyCode::Tab);
    harness.press(KeyCode::Down);
    harness.draw();
    let screen = harness.screen();
    assert_eq!(harness.app.level, MatchLevel::Cautious);
    assert!(screen.contains("(●) Cautious"), "{screen}");
    assert!(screen.contains("Only near-certain matches"), "{screen}");

    harness.press(KeyCode::Up);
    harness.press(KeyCode::Up);
    harness.draw();
    let screen = harness.screen();
    assert_eq!(harness.app.level, MatchLevel::Relaxed);
    assert!(screen.contains("(●) Relaxed"), "{screen}");
    assert!(
        screen.contains("Matches more, for messy naming"),
        "{screen}"
    );

    // Space cycles to the next level rather than stopping at the end.
    harness.press(KeyCode::Char(' '));
    assert_eq!(harness.app.level, MatchLevel::Balanced);
}

#[test]
fn arrows_move_the_highlight_and_space_ticks_the_highlighted_row() {
    let temporary = library();
    let mut harness = Harness::new(temporary.path());
    harness.press(KeyCode::Enter);
    harness.settle(|app| app.preview.is_some());

    let selected = |harness: &Harness| {
        harness
            .app
            .preview
            .as_ref()
            .unwrap()
            .matched_state
            .selected()
    };
    assert_eq!(selected(&harness), Some(0));

    harness.press(KeyCode::Down);
    harness.draw();
    assert_eq!(selected(&harness), Some(1));
    // The detail line spells the highlighted rename out in full.
    let screen = harness.screen();
    assert!(
        screen.contains(
            "[Group] Nebula Archive - S01E02.chs.ass  →  Nebula.Archive.S01E02.1080p.ass"
        ),
        "{screen}"
    );

    // The highlight stops at the last row instead of running off the list.
    harness.press(KeyCode::Down);
    assert_eq!(selected(&harness), Some(1));
    harness.press(KeyCode::Home);
    assert_eq!(selected(&harness), Some(0));
    harness.press(KeyCode::End);
    assert_eq!(selected(&harness), Some(1));

    harness.press(KeyCode::Char(' '));
    harness.draw();
    assert_eq!(harness.app.preview.as_ref().unwrap().ticked, [true, false]);
    assert!(harness.screen().contains("1 of 2 ticked"));
}

#[test]
fn a_cautious_level_matches_less_than_the_default_one() {
    let temporary = fuzzy_library();
    let mut harness = Harness::new(temporary.path());
    harness.press(KeyCode::Enter);
    harness.settle(|app| app.preview.is_some());

    let screen = harness.screen();
    assert!(screen.contains("To rename (1)"), "{screen}");
    assert!(
        screen.contains("Deep Feild Raport.srt → Deep Field Report 2031.srt"),
        "{screen}"
    );
    assert!(screen.contains("fuzzy 0.76"), "{screen}");

    // Back one control to the levels, tighten it, and preview again.
    harness.press(KeyCode::BackTab);
    assert_eq!(harness.app.focus, Focus::Level);
    harness.press(KeyCode::Down);
    assert_eq!(harness.app.level, MatchLevel::Cautious);
    harness.press(KeyCode::Char('p'));
    harness.settle(fresh_preview);

    let screen = harness.screen();
    assert!(screen.contains("To rename (0)"), "{screen}");
    assert!(screen.contains("Nothing to rename"), "{screen}");
    assert!(!harness.app.can_apply());

    harness.press(KeyCode::Tab);
    harness.press(KeyCode::Right);
    harness.draw();
    let screen = harness.screen();
    assert!(screen.contains("No matching video (best 0.76)"), "{screen}");
}

#[test]
fn including_subfolders_finds_the_subtitles_below_the_root() {
    let temporary = nested_library();
    let mut harness = Harness::new(temporary.path());
    harness.press(KeyCode::Enter);
    harness.settle(|app| app.preview.is_some());

    let screen = harness.screen();
    assert!(screen.contains("To rename (1)"), "{screen}");
    assert!(screen.contains("1 folder"), "{screen}");
    assert!(!screen.contains("season 3/"), "{screen}");

    // Turn the subfolder switch on and preview again with p.
    harness.press(KeyCode::Tab);
    harness.press(KeyCode::Tab);
    assert_eq!(harness.app.focus, Focus::Recursive);
    harness.press(KeyCode::Right);
    harness.press(KeyCode::Char('p'));
    harness.settle(fresh_preview);

    let screen = harness.screen();
    assert!(screen.contains("To rename (2)"), "{screen}");
    assert!(screen.contains("2 folders"), "{screen}");
    assert!(
        screen.contains("season 3/[G] Harbour - S03E01.ass → Harbour.S03E01.ass"),
        "{screen}"
    );
}

#[test]
fn applying_with_nothing_ticked_is_refused() {
    let temporary = library();
    let mut harness = Harness::new(temporary.path());
    harness.press(KeyCode::Enter);
    harness.settle(|app| app.preview.is_some());

    harness.press_control(KeyCode::Char('r'));
    harness.press(KeyCode::Char('a'));
    harness.draw();

    assert!(harness.app.modal.is_none());
    assert!(harness.screen().contains("Tick at least one subtitle"));
    assert!(temporary
        .path()
        .join("[Group] Nebula Archive - S01E01.chs.ass")
        .exists());
}

#[test]
fn cancelling_the_confirmation_writes_nothing() {
    let temporary = library();
    let mut harness = Harness::new(temporary.path());
    harness.press(KeyCode::Enter);
    harness.settle(|app| app.preview.is_some());

    harness.press(KeyCode::Char('a'));
    harness.draw();
    let screen = harness.screen();
    assert!(
        screen.contains("About to rename 2 subtitle files."),
        "{screen}"
    );
    assert!(
        screen.contains("Existing files are never overwritten."),
        "{screen}"
    );

    // q closes the dialog; it does not quit the app behind it.
    harness.press(KeyCode::Char('q'));
    harness.draw();
    assert!(harness.app.modal.is_none());
    assert!(!harness.app.should_quit);
    assert!(harness.app.can_apply());
    assert!(temporary
        .path()
        .join("[Group] Nebula Archive - S01E01.chs.ass")
        .exists());
    assert!(!temporary
        .path()
        .join("Nebula.Archive.S01E01.1080p.ass")
        .exists());
}

#[test]
fn previewing_again_after_an_apply_finds_nothing_left_to_do() {
    let temporary = library();
    let mut harness = Harness::new(temporary.path());
    harness.press(KeyCode::Enter);
    harness.settle(|app| app.preview.is_some());

    harness.press(KeyCode::Char('a'));
    harness.press(KeyCode::Enter);
    harness.settle(|app| !app.applying && app.status.text.starts_with("Renamed"));
    assert!(harness.screen().contains("Renamed 2 files"));

    harness.press(KeyCode::Char('p'));
    harness.settle(fresh_preview);

    let screen = harness.screen();
    assert!(screen.contains("To rename (0)"), "{screen}");
    assert!(screen.contains("Skipped (3)"), "{screen}");
    assert!(screen.contains("Nothing to rename"), "{screen}");
    assert!(!harness.app.can_apply());
}

#[test]
fn previewing_a_folder_with_nothing_in_it() {
    let temporary = tempfile::tempdir().unwrap();
    let mut harness = Harness::new(temporary.path());
    harness.press(KeyCode::Enter);
    harness.settle(|app| app.preview.is_some());

    let screen = harness.screen();
    assert!(screen.contains("To rename (0)"), "{screen}");
    assert!(screen.contains("Nothing to rename"), "{screen}");
    assert!(
        screen.contains("No video files in this directory"),
        "{screen}"
    );
    // Nothing was matched, so the keyboard stays where the path is typed.
    assert_eq!(harness.app.focus, Focus::Directory);
    assert!(!harness.app.can_apply());

    fs::write(temporary.path().join("Harbour.S01E01.mkv"), b"video").unwrap();
    harness.press(KeyCode::Enter);
    harness.settle(|app| app.status.text.starts_with("No subtitle"));
    assert!(harness
        .screen()
        .contains("No subtitle files in this directory"));
}

#[test]
fn a_folder_that_is_already_tidy_reports_every_subtitle_as_skipped() {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path();
    fs::write(root.join("Harbour.S01E01.mkv"), b"video").unwrap();
    fs::write(root.join("Harbour.S01E01.srt"), b"subtitle").unwrap();

    let mut harness = Harness::new(root);
    harness.press(KeyCode::Enter);
    harness.settle(|app| app.preview.is_some());
    assert!(harness.screen().contains("To rename (0)"));

    // With nothing to rename the path field keeps the keyboard, so reaching the
    // skipped tab takes an esc first.
    harness.press(KeyCode::Esc);
    harness.press(KeyCode::Right);
    harness.draw();
    assert_eq!(harness.app.tab, Tab::Skipped);
    let screen = harness.screen();
    assert!(screen.contains("Harbour.S01E01.srt"), "{screen}");
    assert!(screen.contains("Filename already matches"), "{screen}");
}

#[test]
fn the_directory_picker_chooses_a_subfolder() {
    let temporary = nested_library();
    let mut harness = Harness::new(temporary.path());
    let typed = harness.app.directory.value();

    harness.press(KeyCode::Esc);
    harness.press(KeyCode::Char('o'));
    harness.draw();
    let screen = harness.screen();
    assert!(screen.contains("Choose a directory"), "{screen}");
    assert!(screen.contains("season 3"), "{screen}");

    // Esc backs out without touching the path.
    harness.press(KeyCode::Esc);
    harness.draw();
    assert!(harness.app.modal.is_none());
    assert_eq!(harness.app.directory.value(), typed);

    harness.press(KeyCode::Char('o'));
    harness.press(KeyCode::Enter);
    harness.press(KeyCode::Char('s'));
    assert!(harness.app.modal.is_none());
    assert_eq!(harness.app.focus, Focus::Directory);
    assert!(harness.app.directory.value().ends_with("season 3"));

    // The chosen folder is scanned on its own.
    harness.press(KeyCode::Enter);
    harness.settle(|app| app.preview.is_some());
    let screen = harness.screen();
    assert!(screen.contains("To rename (1)"), "{screen}");
    assert!(
        screen.contains("[G] Harbour - S03E01.ass → Harbour.S03E01.ass"),
        "{screen}"
    );
}

#[test]
fn vim_letters_stand_in_for_the_arrows_over_the_setup_controls() {
    let temporary = tempfile::tempdir().unwrap();
    let mut harness = Harness::new(temporary.path());

    // h and l set a switch the way ← and → do, rather than toggling it.
    harness.press(KeyCode::Tab);
    harness.press(KeyCode::Char('l'));
    harness.draw();
    assert!(harness.app.recursive);
    assert!(harness.screen().contains("[✓] Include subfolders"));
    harness.press(KeyCode::Char('h'));
    harness.draw();
    assert!(!harness.app.recursive);
    assert!(harness.screen().contains("[ ] Include subfolders"));

    // j and k step between the controls, exactly as down and up do.
    harness.press(KeyCode::Char('j'));
    assert_eq!(harness.app.focus, Focus::Strict);
    harness.press(KeyCode::Char('k'));
    assert_eq!(harness.app.focus, Focus::Recursive);

    harness.press(KeyCode::Tab);
    harness.press(KeyCode::Tab);
    assert_eq!(harness.app.focus, Focus::Level);
    harness.press(KeyCode::Char('j'));
    harness.draw();
    assert_eq!(harness.app.level, MatchLevel::Cautious);
    assert!(harness.screen().contains("(●) Cautious"));
    harness.press(KeyCode::Char('k'));
    assert_eq!(harness.app.level, MatchLevel::Balanced);

    // g and G are home and end: the ends of the group, not the ends of the form.
    harness.press(KeyCode::Char('g'));
    harness.draw();
    assert_eq!(harness.app.level, MatchLevel::Relaxed);
    assert!(harness.screen().contains("(●) Relaxed"));
    harness.press(KeyCode::Char('G'));
    assert_eq!(harness.app.level, MatchLevel::Cautious);
    assert_eq!(harness.app.focus, Focus::Level);
}

#[test]
fn vim_letters_move_the_highlight_and_switch_tabs_in_the_results() {
    let temporary = library();
    let mut harness = Harness::new(temporary.path());
    harness.press(KeyCode::Enter);
    harness.settle(|app| app.preview.is_some());

    let selected = |harness: &Harness| {
        harness
            .app
            .preview
            .as_ref()
            .unwrap()
            .matched_state
            .selected()
    };
    assert_eq!(harness.app.focus, Focus::Results);

    harness.press(KeyCode::Char('j'));
    harness.draw();
    assert_eq!(selected(&harness), Some(1));
    assert!(harness
        .screen()
        .contains("[Group] Nebula Archive - S01E02.chs.ass  →  Nebula.Archive.S01E02.1080p.ass"));
    harness.press(KeyCode::Char('k'));
    assert_eq!(selected(&harness), Some(0));
    harness.press(KeyCode::Char('G'));
    assert_eq!(selected(&harness), Some(1));
    harness.press(KeyCode::Char('g'));
    assert_eq!(selected(&harness), Some(0));

    // h and l are the tab keys here, the same as ← and →.
    harness.press(KeyCode::Char('l'));
    harness.draw();
    assert_eq!(harness.app.tab, Tab::Skipped);
    assert!(harness.screen().contains("Unrelated.Bonus.srt"));
    harness.press(KeyCode::Char('h'));
    harness.draw();
    assert_eq!(harness.app.tab, Tab::Matched);
    assert!(harness.screen().contains("2 of 2 ticked"));
}

#[test]
fn the_page_keys_jump_by_half_a_screen_and_a_whole_one_and_stop_at_the_ends() {
    let temporary = long_library();
    let mut harness = Harness::new(temporary.path());
    harness.press(KeyCode::Enter);
    harness.settle(|app| app.preview.is_some());

    let selected = |harness: &Harness| {
        harness
            .app
            .preview
            .as_ref()
            .unwrap()
            .matched_state
            .selected()
    };
    assert!(harness.screen().contains("To rename (14)"));
    assert_eq!(selected(&harness), Some(0));

    harness.press_control(KeyCode::Char('d'));
    harness.draw();
    assert_eq!(selected(&harness), Some(5));
    // Half a page is five rows, and the detail line proves which row that is.
    assert!(harness
        .screen()
        .contains("[G] Harbour - S01E06.chs.ass  →  Harbour.S01E06.ass"));

    harness.press_control(KeyCode::Char('u'));
    assert_eq!(selected(&harness), Some(0));

    harness.press_control(KeyCode::Char('f'));
    harness.draw();
    assert_eq!(selected(&harness), Some(10));
    assert!(harness
        .screen()
        .contains("[G] Harbour - S01E11.chs.ass  →  Harbour.S01E11.ass"));

    harness.press_control(KeyCode::Char('b'));
    assert_eq!(selected(&harness), Some(0));

    // Both directions stop at the end of the list instead of running off it.
    harness.press_control(KeyCode::Char('b'));
    assert_eq!(selected(&harness), Some(0));
    harness.press_control(KeyCode::Char('f'));
    harness.press_control(KeyCode::Char('f'));
    assert_eq!(selected(&harness), Some(13));
    harness.press_control(KeyCode::Char('d'));
    assert_eq!(selected(&harness), Some(13));
}

#[test]
fn the_match_levels_hand_the_keyboard_on_at_either_end() {
    let temporary = tempfile::tempdir().unwrap();
    let mut harness = Harness::new(temporary.path());
    harness.app.focus = Focus::Level;

    // Off the top of the group is the control above it, not a dead key: three
    // radios that up and down cannot leave would trap the keyboard.
    harness.press(KeyCode::Up);
    assert_eq!(harness.app.level, MatchLevel::Relaxed);
    assert_eq!(harness.app.focus, Focus::Level);
    harness.press(KeyCode::Up);
    assert_eq!(harness.app.focus, Focus::Strict);
    assert_eq!(harness.app.level, MatchLevel::Relaxed);

    harness.app.focus = Focus::Level;
    harness.press(KeyCode::Down);
    harness.press(KeyCode::Down);
    assert_eq!(harness.app.level, MatchLevel::Cautious);
    assert_eq!(harness.app.focus, Focus::Level);
    harness.press(KeyCode::Down);
    assert_eq!(harness.app.focus, Focus::Results);
    assert_eq!(harness.app.level, MatchLevel::Cautious);
}

#[test]
fn sideways_and_jump_keys_stay_inside_the_match_levels() {
    let temporary = tempfile::tempdir().unwrap();
    let mut harness = Harness::new(temporary.path());
    harness.app.focus = Focus::Level;

    // Left at the top of the group holds still: sideways sets the value, so it
    // must never be the key that loses the group.
    harness.press(KeyCode::Left);
    harness.press(KeyCode::Left);
    harness.draw();
    assert_eq!(harness.app.level, MatchLevel::Relaxed);
    assert_eq!(harness.app.focus, Focus::Level);
    assert!(harness.screen().contains("(●) Relaxed"));

    harness.press(KeyCode::Right);
    harness.press(KeyCode::Right);
    harness.press(KeyCode::Right);
    harness.draw();
    assert_eq!(harness.app.level, MatchLevel::Cautious);
    assert_eq!(harness.app.focus, Focus::Level);
    assert!(harness.screen().contains("(●) Cautious"));

    // A jump lands on the first or last level rather than stepping out.
    harness.press(KeyCode::Home);
    assert_eq!(harness.app.level, MatchLevel::Relaxed);
    assert_eq!(harness.app.focus, Focus::Level);
    harness.press(KeyCode::End);
    assert_eq!(harness.app.level, MatchLevel::Cautious);
    assert_eq!(harness.app.focus, Focus::Level);
    harness.press(KeyCode::Char('g'));
    assert_eq!(harness.app.level, MatchLevel::Relaxed);
    assert_eq!(harness.app.focus, Focus::Level);
    harness.press(KeyCode::Char('G'));
    assert_eq!(harness.app.level, MatchLevel::Cautious);
    assert_eq!(harness.app.focus, Focus::Level);
}

#[test]
fn up_and_down_leave_the_path_field_and_i_comes_back_to_it() {
    let temporary = tempfile::tempdir().unwrap();
    let mut harness = Harness::new(temporary.path());

    // The field is the first row of the form, so down is the next control and up
    // wraps round to the far end rather than doing nothing.
    harness.press(KeyCode::Down);
    assert_eq!(harness.app.focus, Focus::Recursive);
    harness.press(KeyCode::Up);
    assert_eq!(harness.app.focus, Focus::Directory);
    harness.press(KeyCode::Up);
    assert_eq!(harness.app.focus, Focus::Results);

    // Nothing was typed on the way: those keys navigated, they did not edit.
    assert_eq!(
        harness.app.directory.value(),
        temporary.path().to_string_lossy()
    );

    harness.press(KeyCode::Char('i'));
    assert_eq!(harness.app.focus, Focus::Directory);
    harness.press(KeyCode::Char('i'));
    assert!(harness.app.directory.value().ends_with('i'));
}

#[test]
fn the_path_field_answers_to_the_shell_editing_keys() {
    let temporary = tempfile::tempdir().unwrap();
    let mut harness = Harness::new(temporary.path());

    // The cursor is not on screen anywhere, so where it sits is shown by where
    // the next typed character lands.
    harness.app.directory.set_value("/tmp/shows");
    harness.press_control(KeyCode::Char('a'));
    harness.press(KeyCode::Char('~'));
    assert_eq!(harness.app.directory.value(), "~/tmp/shows");
    harness.press_control(KeyCode::Char('e'));
    harness.press(KeyCode::Char('!'));
    assert_eq!(harness.app.directory.value(), "~/tmp/shows!");

    // Control+K drops the tail from wherever the cursor is.
    harness.app.directory.set_value("/tmp/shows/season");
    for _ in 0..6 {
        harness.press(KeyCode::Left);
    }
    harness.press_control(KeyCode::Char('k'));
    assert_eq!(harness.app.directory.value(), "/tmp/shows/");

    // Control+W takes a whole path segment, not a word.
    harness.app.directory.set_value("/tmp/shows/season one/");
    harness.press_control(KeyCode::Char('w'));
    assert_eq!(harness.app.directory.value(), "/tmp/shows/");

    harness.press_control(KeyCode::Char('u'));
    assert!(harness.app.directory.is_empty());
    harness.press(KeyCode::Enter);
    harness.draw();
    assert!(harness.screen().contains("Enter a directory path first"));
}

#[test]
fn a_control_held_letter_never_types_itself_into_the_path() {
    let temporary = tempfile::tempdir().unwrap();
    let mut harness = Harness::new(temporary.path());
    harness.app.directory.set_value("/tmp/shows");

    // These are commands elsewhere in the interface; in the field they are simply
    // not text, and a path that quietly grows an "x" is a path that will not scan.
    for character in ['x', 'p', 'o', 'z', 'g'] {
        harness.press_control(KeyCode::Char(character));
    }
    assert_eq!(harness.app.directory.value(), "/tmp/shows");
    assert!(harness.app.modal.is_none());
    assert!(harness.app.preview.is_none());
    assert_eq!(harness.app.focus, Focus::Directory);
}

#[test]
fn space_says_why_nothing_was_ticked_instead_of_going_quiet() {
    let temporary = library();
    let mut harness = Harness::new(temporary.path());
    harness.press(KeyCode::Enter);
    harness.settle(|app| app.preview.is_some());

    // Skipped rows have no checkbox, and a key that does nothing reads as broken.
    harness.press(KeyCode::Right);
    harness.press(KeyCode::Char(' '));
    harness.draw();
    let screen = harness.screen();
    assert!(
        screen.contains("Skipped files are not renamed — press ← for the renames"),
        "{screen}"
    );
    assert_eq!(harness.app.preview.as_ref().unwrap().ticked, [true, true]);

    // The same before there is anything to tick at all, with the way out spelt out.
    let empty = tempfile::tempdir().unwrap();
    let mut harness = Harness::new(empty.path());
    harness.press(KeyCode::Esc);
    harness.press(KeyCode::Char(' '));
    harness.draw();
    let screen = harness.screen();
    assert!(
        screen.contains("Nothing to tick yet — p to preview, d for a demo"),
        "{screen}"
    );
}

#[test]
fn the_footer_names_the_keys_that_the_focused_control_answers_to() {
    let temporary = tempfile::tempdir().unwrap();
    let mut harness = Harness::new(temporary.path());

    harness.draw();
    assert!(harness.footer().contains("enter preview"));

    harness.press(KeyCode::Tab);
    harness.draw();
    let footer = harness.footer();
    assert!(footer.contains("space toggle"), "{footer}");
    assert!(!footer.contains("space tick"), "{footer}");

    harness.press(KeyCode::Tab);
    harness.press(KeyCode::Tab);
    assert_eq!(harness.app.focus, Focus::Level);
    harness.draw();
    let footer = harness.footer();
    assert!(footer.contains("↑↓ choose"), "{footer}");
    assert!(!footer.contains("space toggle"), "{footer}");

    harness.press(KeyCode::Tab);
    harness.draw();
    let footer = harness.footer();
    assert!(footer.contains("space tick"), "{footer}");
    assert!(footer.contains("←→ tabs"), "{footer}");
    // The whole list fits at this width; the narrow case below is what trims it.
    assert!(footer.contains("d demo"), "{footer}");
}

#[test]
fn the_footer_drops_its_least_important_hints_before_help_and_quit() {
    let temporary = tempfile::tempdir().unwrap();
    let mut harness = Harness::new(temporary.path());
    harness.terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
    harness.press(KeyCode::Esc);
    assert_eq!(harness.app.focus, Focus::Results);
    harness.draw();

    // The results have the longest hint list, so this is where the line has to
    // give: the tail goes first and the two escape hatches always stay.
    let footer = harness.footer();
    assert!(footer.contains("? help"), "{footer}");
    assert!(footer.contains("q quit"), "{footer}");
    assert!(footer.contains("space tick"), "{footer}");
    assert!(!footer.contains("d demo"), "{footer}");
}

#[test]
fn the_directory_picker_takes_the_vim_keys_and_space() {
    let temporary = picker_library();
    let mut harness = Harness::new(temporary.path());
    harness.press(KeyCode::Esc);
    harness.press(KeyCode::Char('o'));
    harness.draw();
    assert!(harness.screen().contains("Choose a directory"));

    // The highlight is a colour, so where it sits only shows in what l opens.
    // j and k walk the listing: two down and one back up is "season 2".
    harness.press(KeyCode::Char('j'));
    harness.press(KeyCode::Char('j'));
    harness.press(KeyCode::Char('k'));
    harness.press(KeyCode::Char('l'));
    harness.draw();
    let screen = harness.screen();
    assert!(screen.contains("season 2"), "{screen}");
    assert!(screen.contains("No subfolders here"), "{screen}");

    // h comes back up, the same as ←, and g goes to the top of the listing.
    harness.press(KeyCode::Char('h'));
    harness.press(KeyCode::Char('g'));
    harness.press(KeyCode::Char('l'));
    harness.draw();
    let screen = harness.screen();
    assert!(screen.contains("extras"), "{screen}");

    // G goes to the bottom of it.
    harness.press(KeyCode::Char('h'));
    harness.press(KeyCode::Char('G'));
    harness.press(KeyCode::Char('l'));

    // Space takes the open folder, just as s does.
    harness.press(KeyCode::Char(' '));
    assert!(harness.app.modal.is_none());
    assert_eq!(harness.app.focus, Focus::Directory);
    assert!(
        harness.app.directory.value().ends_with("season 3"),
        "{}",
        harness.app.directory.value()
    );
}
