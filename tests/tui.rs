//! End-to-end tests of the terminal interface, driven by keystrokes and read
//! back from a rendered frame — the same path a person takes.

use std::fs;
use std::path::Path;
use std::time::{Duration, Instant};

use ratatui::backend::TestBackend;
use ratatui::crossterm::event::{
    KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use ratatui::layout::Rect;
use ratatui::Terminal;

use beaver::presentation::MatchLevel;
use beaver::tui::app::{App, Control, Hit, Modal, Step};
use beaver::tui::ui;

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

    /// A harness whose path field starts empty, for typing into.
    fn empty() -> Self {
        Self::new(Path::new(""))
    }

    /// Which proposed rename holds the highlight.
    fn selected(&self) -> Option<usize> {
        self.app.preview.as_ref().unwrap().state.selected()
    }

    fn press(&mut self, code: KeyCode) {
        self.app.handle_key(KeyEvent::new(code, KeyModifiers::NONE));
    }

    fn press_control(&mut self, code: KeyCode) {
        self.app
            .handle_key(KeyEvent::new(code, KeyModifiers::CONTROL));
    }

    /// Type a whole string into whatever holds the keyboard.
    fn type_text(&mut self, text: &str) {
        for character in text.chars() {
            self.press(KeyCode::Char(character));
        }
    }

    /// Step 1 to a settled preview, which is where most tests start.
    fn walk_to_preview(&mut self) {
        self.press(KeyCode::Enter);
        assert_eq!(self.app.step, Step::Rules, "{}", self.app.status.text);
        self.press(KeyCode::Enter);
        self.settle(|app| !app.scanning && app.preview.is_some());
    }

    /// The rectangle the last drawn frame registered for `hit`.
    fn rect_for(&self, hit: Hit) -> Rect {
        self.app
            .hits
            .iter()
            .find(|(_, found)| *found == hit)
            .map(|(rect, _)| *rect)
            .unwrap_or_else(|| panic!("{hit:?} is not on screen"))
    }

    fn mouse(&mut self, kind: MouseEventKind, hit: Hit) {
        self.draw();
        let rect = self.rect_for(hit);
        self.app.handle_mouse(MouseEvent {
            kind,
            column: rect.x + rect.width / 2,
            row: rect.y + rect.height / 2,
            modifiers: KeyModifiers::NONE,
        });
    }

    fn click(&mut self, hit: Hit) {
        self.mouse(MouseEventKind::Down(MouseButton::Left), hit);
    }

    fn hover(&mut self, hit: Hit) {
        self.mouse(MouseEventKind::Moved, hit);
    }

    /// A wheel notch over the middle of the screen.
    fn wheel(&mut self, kind: MouseEventKind) {
        self.draw();
        self.app.handle_mouse(MouseEvent {
            kind,
            column: 60,
            row: 16,
            modifiers: KeyModifiers::NONE,
        });
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

    /// The bottom row on its own: the status line and the hints beside it.
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
    !app.scanning && app.preview.is_some()
}

#[test]
fn the_whole_wizard_types_previews_ticks_and_renames() {
    let temporary = library();
    let mut harness = Harness::empty();
    harness.type_text(&temporary.path().to_string_lossy());

    harness.press(KeyCode::Enter);
    assert_eq!(harness.app.step, Step::Rules);
    harness.press(KeyCode::Enter);
    harness.settle(fresh_preview);
    assert_eq!(harness.app.step, Step::Preview);

    let screen = harness.screen();
    assert!(screen.contains("2 of 2 ticked"), "{screen}");
    assert!(screen.contains("episode S01E01"), "{screen}");
    assert!(screen.contains("episode S01E02"), "{screen}");
    // The list trims from the left, so the tails are what is on screen.
    assert!(
        screen.contains("Nebula.Archive.S01E01.1080p.ass"),
        "{screen}"
    );
    assert!(
        screen.contains("Nebula.Archive.S01E02.1080p.ass"),
        "{screen}"
    );
    assert!(screen.contains("1 skipped"), "{screen}");
    assert!(harness.footer().contains("Preview ready"));
    assert!(harness.app.is_focused(Control::List));
    assert!(harness.app.can_apply());

    // Untick the highlighted rename; only the other one may be applied.
    harness.press(KeyCode::Char(' '));
    harness.draw();
    assert!(harness.screen().contains("1 of 2 ticked"));

    harness.press(KeyCode::Char('a'));
    harness.draw();
    let screen = harness.screen();
    assert!(screen.contains("Confirm apply"), "{screen}");
    assert!(screen.contains("Rename 1 subtitle on disk."), "{screen}");

    harness.press(KeyCode::Enter);
    harness.settle(|app| !app.applying && app.outcome.is_some());
    assert_eq!(harness.app.step, Step::Apply);
    assert!(harness.app.preview.is_none());

    let root = temporary.path();
    assert!(root.join("Nebula.Archive.S01E02.1080p.ass").exists());
    assert!(root
        .join("[Group] Nebula Archive - S01E01.chs.ass")
        .exists());
    let screen = harness.screen();
    assert!(screen.contains("Renamed 1 file"), "{screen}");
    assert!(screen.contains("Start over"), "{screen}");
}

#[test]
fn the_wizard_walks_both_ways() {
    // An empty path is not a folder, so step one refuses to be left.
    let mut harness = Harness::empty();
    harness.press(KeyCode::Enter);
    harness.draw();
    assert_eq!(harness.app.step, Step::Folder);
    assert!(harness.footer().contains("Enter a folder path first"));

    // Nor is a path that points at nothing.
    let mut harness = Harness::new(Path::new("/no/such/folder/here"));
    harness.press(KeyCode::Enter);
    harness.draw();
    assert_eq!(harness.app.step, Step::Folder);
    assert!(
        harness.footer().contains("Not a folder"),
        "{}",
        harness.footer()
    );

    let temporary = library();
    let mut harness = Harness::new(temporary.path());
    harness.press(KeyCode::Enter);
    assert_eq!(harness.app.step, Step::Rules);
    harness.press(KeyCode::Left);
    assert_eq!(harness.app.step, Step::Folder);

    harness.walk_to_preview();
    assert_eq!(harness.app.step, Step::Preview);
    harness.press(KeyCode::Left);
    harness.draw();
    assert_eq!(harness.app.step, Step::Rules);
    assert!(harness.screen().contains("2 · Rules"));
}

#[test]
fn changing_a_rule_drops_the_preview_and_rescans() {
    let temporary = library();
    let mut harness = Harness::new(temporary.path());
    harness.walk_to_preview();
    assert!(harness.app.can_apply());

    // Back to the rules, then onto the subfolder switch and flip it.
    harness.press(KeyCode::Left);
    harness.press(KeyCode::Tab);
    assert!(harness.app.is_focused(Control::Recursive));
    harness.press(KeyCode::Char(' '));
    harness.draw();
    assert!(harness.app.recursive);
    assert!(harness.app.preview.is_none());
    assert!(harness.screen().contains("[✓] Include subfolders"));

    // The level does the same.
    harness.press(KeyCode::Enter);
    harness.settle(fresh_preview);
    harness.press(KeyCode::Left);
    assert!(harness.app.is_focused(Control::Level));
    harness.press(KeyCode::Down);
    assert_eq!(harness.app.level, MatchLevel::Cautious);
    assert!(harness.app.preview.is_none());

    harness.press(KeyCode::Enter);
    harness.settle(fresh_preview);
    assert_eq!(harness.app.step, Step::Preview);
    assert!(harness.screen().contains("2 of 2 ticked"));
}

#[test]
fn bulk_ticking_moves_the_summary_line() {
    let temporary = library();
    let mut harness = Harness::new(temporary.path());
    harness.walk_to_preview();

    harness.press_control(KeyCode::Char('r'));
    harness.draw();
    assert!(harness.screen().contains("0 of 2 ticked"));
    assert!(!harness.app.can_apply());

    harness.press_control(KeyCode::Char('a'));
    harness.draw();
    assert!(harness.screen().contains("2 of 2 ticked"));
    assert!(harness.app.can_apply());
}

#[test]
fn the_skipped_list_opens_and_closes() {
    let temporary = library();
    let mut harness = Harness::new(temporary.path());
    harness.walk_to_preview();

    harness.press(KeyCode::Char('s'));
    harness.draw();
    let screen = harness.screen();
    assert!(screen.contains("Skipped (1)"), "{screen}");
    assert!(screen.contains("Unrelated.Bonus.srt"), "{screen}");
    assert!(screen.contains("No matching video"), "{screen}");

    harness.press(KeyCode::Esc);
    harness.draw();
    assert!(harness.app.modal.is_none());
    assert!(!harness.screen().contains("Skipped (1)"));
}

#[test]
fn the_match_level_decides_whether_a_fuzzy_pair_matches() {
    let temporary = fuzzy_library();
    let mut harness = Harness::new(temporary.path());
    harness.walk_to_preview();

    let screen = harness.screen();
    assert!(screen.contains("1 of 1 ticked"), "{screen}");
    assert!(screen.contains("fuzzy 0.76"), "{screen}");
    assert!(screen.contains("Deep Field Report 2031.srt"), "{screen}");

    // Cautious asks for more than 0.76, so the pair falls out.
    harness.press(KeyCode::Left);
    harness.press(KeyCode::Down);
    assert_eq!(harness.app.level, MatchLevel::Cautious);
    harness.press(KeyCode::Enter);
    harness.settle(fresh_preview);
    let screen = harness.screen();
    assert!(screen.contains("0 of 0 ticked"), "{screen}");
    assert!(
        screen.contains("Nothing matched — press ← and loosen the match level"),
        "{screen}"
    );
    assert!(!harness.app.can_apply());

    // Relaxed takes it back.
    harness.press(KeyCode::Left);
    harness.press(KeyCode::Up);
    harness.press(KeyCode::Up);
    assert_eq!(harness.app.level, MatchLevel::Relaxed);
    harness.press(KeyCode::Enter);
    harness.settle(fresh_preview);
    assert!(harness.screen().contains("1 of 1 ticked"));
}

#[test]
fn including_subfolders_finds_the_subtitles_below_the_root() {
    let temporary = nested_library();
    let mut harness = Harness::new(temporary.path());
    harness.walk_to_preview();

    let screen = harness.screen();
    assert!(screen.contains("1 of 1 ticked"), "{screen}");
    assert!(screen.contains("Harbour.S02E01.ass"), "{screen}");

    harness.press(KeyCode::Left);
    harness.press(KeyCode::Tab);
    assert!(harness.app.is_focused(Control::Recursive));
    harness.press(KeyCode::Char(' '));
    harness.press(KeyCode::Enter);
    harness.settle(fresh_preview);

    let screen = harness.screen();
    assert!(screen.contains("2 of 2 ticked"), "{screen}");
    assert!(screen.contains("Harbour.S03E01.ass"), "{screen}");
}

#[test]
fn the_folder_picker_walks_and_fills_the_path_in() {
    let temporary = picker_library();
    let mut harness = Harness::new(temporary.path());

    harness.press(KeyCode::Esc);
    harness.press(KeyCode::Char('o'));
    harness.draw();
    let screen = harness.screen();
    assert!(screen.contains("Browse"), "{screen}");
    assert!(screen.contains("season 1/"), "{screen}");
    assert!(screen.contains("season 3/"), "{screen}");

    // Two down and one back up is "season 2", which Enter opens.
    harness.press(KeyCode::Down);
    harness.press(KeyCode::Down);
    harness.press(KeyCode::Up);
    harness.press(KeyCode::Enter);
    harness.draw();
    let screen = harness.screen();
    assert!(screen.contains("season 2"), "{screen}");
    assert!(screen.contains("No subfolders here"), "{screen}");

    harness.press(KeyCode::Char('s'));
    assert!(harness.app.modal.is_none());
    assert_eq!(harness.app.step, Step::Folder);
    assert!(
        harness.app.directory.value().ends_with("season 2"),
        "{}",
        harness.app.directory.value()
    );

    // The Use this folder button does the same as s.
    harness.press(KeyCode::Esc);
    harness.press(KeyCode::Char('o'));
    harness.click(Hit::PickerUse);
    assert!(harness.app.modal.is_none());
    assert!(harness.app.directory.value().ends_with("season 2"));
}

#[test]
fn the_help_modal_lists_the_shortcuts_and_closes() {
    let temporary = tempfile::tempdir().unwrap();
    let mut harness = Harness::new(temporary.path());
    harness.press(KeyCode::Esc);
    harness.press(KeyCode::Char('?'));
    harness.draw();

    let screen = harness.screen();
    assert!(screen.contains("Keyboard shortcuts"), "{screen}");
    assert!(screen.contains("Tick everything / nothing"), "{screen}");
    assert!(
        screen.contains("Back and forward through the four steps"),
        "{screen}"
    );

    harness.press(KeyCode::Esc);
    harness.draw();
    assert!(harness.app.modal.is_none());
    assert!(!harness.screen().contains("Keyboard shortcuts"));
}

#[test]
fn the_mouse_selects_ticks_toggles_and_steps_back() {
    let temporary = library();
    let mut harness = Harness::new(temporary.path());
    harness.walk_to_preview();

    // The first click moves the highlight without touching any tick.
    harness.click(Hit::Row(1));
    assert!(harness.app.is_focused(Control::List));
    assert_eq!(harness.selected(), Some(1));
    assert!(harness.app.preview.as_ref().unwrap().ticked[1]);

    // The second click on the same row unticks it.
    harness.click(Hit::Row(1));
    harness.draw();
    assert!(!harness.app.preview.as_ref().unwrap().ticked[1]);
    assert!(harness.screen().contains("1 of 2 ticked"));

    // Hovering a button changes nothing and panics at nothing.
    harness.hover(Hit::Control(Control::Advance));
    harness.draw();
    assert_eq!(harness.selected(), Some(1));

    // The wheel moves the highlight in the list underneath the pointer.
    harness.wheel(MouseEventKind::ScrollUp);
    assert_eq!(harness.selected(), Some(0));

    // A dot behind the current step walks back to it.
    harness.click(Hit::Dot(0));
    assert_eq!(harness.app.step, Step::Folder);

    // The subfolder switch flips under the pointer.
    harness.press(KeyCode::Enter);
    assert_eq!(harness.app.step, Step::Rules);
    harness.click(Hit::Control(Control::Recursive));
    harness.draw();
    assert!(harness.app.recursive);
    assert!(harness.screen().contains("[✓] Include subfolders"));
    harness.click(Hit::Control(Control::Recursive));
    harness.draw();
    assert!(!harness.app.recursive);
    assert!(harness.screen().contains("[ ] Include subfolders"));
}

#[test]
fn the_step_bar_names_all_four_steps() {
    let temporary = library();
    let mut harness = Harness::new(temporary.path());
    harness.draw();
    let screen = harness.screen();
    for label in ["Folder", "Rules", "Preview", "Apply"] {
        assert!(screen.contains(label), "{label} missing from {screen}");
    }
    assert!(screen.contains("1 · Folder"), "{screen}");
}

#[test]
fn every_step_still_renders_in_an_eighty_by_twenty_four_terminal() {
    let temporary = library();
    let mut harness = Harness::new(temporary.path());
    harness.terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();

    harness.draw();
    let screen = harness.screen();
    assert!(screen.contains("1 · Folder"), "{screen}");
    assert!(screen.contains("Next →"), "{screen}");

    harness.press(KeyCode::Enter);
    harness.draw();
    let screen = harness.screen();
    assert!(screen.contains("2 · Rules"), "{screen}");
    assert!(screen.contains("Preview →"), "{screen}");
    assert!(screen.contains("Include subfolders"), "{screen}");

    harness.press(KeyCode::Enter);
    harness.settle(fresh_preview);
    let screen = harness.screen();
    assert!(screen.contains("3 · Preview"), "{screen}");
    // Not bare "Apply": the step bar carries that word on every step.
    assert!(screen.contains("Apply (a)"), "{screen}");
    assert!(screen.contains("2 of 2 ticked"), "{screen}");

    harness.press(KeyCode::Char('a'));
    harness.press(KeyCode::Enter);
    harness.settle(|app| !app.applying && app.outcome.is_some());
    let screen = harness.screen();
    assert!(screen.contains("4 · Apply"), "{screen}");
    assert!(screen.contains("Start over"), "{screen}");
    assert!(screen.contains("Renamed 2 files"), "{screen}");
}

#[test]
fn the_jump_keys_keep_the_highlight_inside_a_long_list() {
    let temporary = long_library();
    let mut harness = Harness::new(temporary.path());
    harness.walk_to_preview();

    assert!(harness.screen().contains("14 of 14 ticked"));
    assert_eq!(harness.selected(), Some(0));

    harness.press(KeyCode::Char('G'));
    assert_eq!(harness.selected(), Some(13));
    harness.press(KeyCode::Char('G'));
    assert_eq!(harness.selected(), Some(13));
    harness.press(KeyCode::Char('g'));
    assert_eq!(harness.selected(), Some(0));

    harness.press_control(KeyCode::Char('d'));
    assert_eq!(harness.selected(), Some(5));
    harness.press_control(KeyCode::Char('u'));
    assert_eq!(harness.selected(), Some(0));
    harness.press_control(KeyCode::Char('u'));
    assert_eq!(harness.selected(), Some(0));

    harness.press(KeyCode::PageDown);
    assert_eq!(harness.selected(), Some(10));
    harness.press(KeyCode::PageDown);
    assert_eq!(harness.selected(), Some(13));
    harness.press(KeyCode::PageUp);
    assert_eq!(harness.selected(), Some(3));
    harness.press(KeyCode::PageUp);
    assert_eq!(harness.selected(), Some(0));

    harness.draw();
    assert!(harness.screen().contains("Harbour.S01E01.ass"));
}

#[test]
fn a_modal_is_reachable_and_dismissable_by_the_mouse() {
    let temporary = library();
    let mut harness = Harness::new(temporary.path());
    harness.draw();

    harness.click(Hit::Help);
    assert!(matches!(harness.app.modal, Some(Modal::Help)));
    harness.click(Hit::CloseModal);
    assert!(harness.app.modal.is_none());

    harness.click(Hit::Quit);
    assert!(harness.app.should_quit);
}
