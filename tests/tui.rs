//! End-to-end tests of the terminal interface, driven by keystrokes and read
//! back from a rendered frame — the same path a person takes.

use std::fs;
use std::path::Path;
use std::time::{Duration, Instant};

use ratatui::backend::TestBackend;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::Terminal;

use rename_subtitles::tui::app::{App, Focus, Tab};
use rename_subtitles::tui::ui;

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
