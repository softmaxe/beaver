//! State and behaviour of the terminal interface.
//!
//! The shape of the screen mirrors the shape of the task: point at a directory,
//! read a preview that touches nothing, then tick off the renames you actually
//! want. Only the apply step writes, and it re-checks the filesystem first.
//!
//! Scanning and applying both run on a worker thread and report back through a
//! channel, so a large recursive directory never freezes the interface.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread;

use ratatui::crossterm::event::{
    KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use ratatui::layout::{Position, Rect};
use ratatui::widgets::{ListState, TableState};

use crate::applying::{
    apply_operations, prepare_operations, ApplyResult, ApplyStatus, PlanChanged, PreparedOperation,
};
use crate::paths::{display_path, file_name};
use crate::planning::{plan_directory, PlanOptions, RenamePlan};
use crate::presentation::{demo_plan, plural, MatchLevel};
use crate::tui::input::TextInput;
use crate::tui::picker::Picker;

/// How many renames the confirmation dialog spells out before summarising.
const CONFIRM_EXAMPLE_LIMIT: usize = 5;

/// Rows a page key moves through a list, and half of it for `ctrl+d` / `ctrl+u`.
const PAGE: isize = 10;
const HALF_PAGE: isize = 5;

/// Rows one notch of the mouse wheel moves.
const WHEEL: isize = 3;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Focus {
    Directory,
    Recursive,
    Strict,
    Level,
    Results,
}

impl Focus {
    const ORDER: [Self; 5] = [
        Self::Directory,
        Self::Recursive,
        Self::Strict,
        Self::Level,
        Self::Results,
    ];

    fn step(self, delta: isize) -> Self {
        let count = Self::ORDER.len() as isize;
        let current = Self::ORDER
            .iter()
            .position(|item| *item == self)
            .unwrap_or(0) as isize;
        Self::ORDER[(current + delta).rem_euclid(count) as usize]
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tab {
    Matched,
    Skipped,
}

/// What a left click can land on, registered by the draw pass as rectangles.
///
/// The draw pass knows where everything sits, so it records each clickable
/// region as it renders; the mouse handler then looks the position up instead
/// of recomputing layout. Rows carry their index, so one variant per kind of
/// target is all the dispatch needs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hit {
    Directory,
    BrowseButton,
    Recursive,
    Strict,
    Level(usize),
    PreviewButton,
    ApplyButton,
    DemoButton,
    HelpButton,
    QuitButton,
    TickAll,
    TickNone,
    Tab(Tab),
    MatchedRow(usize),
    SkippedRow(usize),
    PickerRow(usize),
    PickerParent,
    PickerUse,
    PickerCancel,
    ConfirmApply,
    ConfirmCancel,
    HelpClose,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StatusKind {
    Ready,
    Working,
    Success,
    Demo,
    Error,
}

pub struct Status {
    pub text: String,
    pub kind: StatusKind,
}

impl Status {
    fn new(text: impl Into<String>, kind: StatusKind) -> Self {
        Self {
            text: text.into(),
            kind,
        }
    }
}

/// A preview: a plan, the checkboxes over it, and whether it still describes disk.
pub struct Preview {
    pub plan: RenamePlan,
    pub prepared: Vec<PreparedOperation>,
    pub ticked: Vec<bool>,
    pub matched_state: ListState,
    pub skipped_state: TableState,
    pub is_demo: bool,
    /// Set when an option changed, or when the files moved under the preview.
    pub stale: bool,
}

impl Preview {
    fn new(plan: RenamePlan, is_demo: bool) -> Self {
        let prepared = prepare_operations(&plan);
        let mut matched_state = ListState::default();
        if !prepared.is_empty() {
            matched_state.select(Some(0));
        }
        let mut skipped_state = TableState::default();
        if !plan.skipped.is_empty() {
            skipped_state.select(Some(0));
        }
        Self {
            ticked: vec![true; prepared.len()],
            prepared,
            plan,
            matched_state,
            skipped_state,
            is_demo,
            stale: false,
        }
    }

    pub fn ticked_count(&self) -> usize {
        self.ticked.iter().filter(|ticked| **ticked).count()
    }

    fn chosen(&self) -> Vec<PreparedOperation> {
        self.prepared
            .iter()
            .zip(&self.ticked)
            .filter(|(_, ticked)| **ticked)
            .map(|(prepared, _)| prepared.clone())
            .collect()
    }
}

pub enum Modal {
    Help,
    Confirm { count: usize, examples: Vec<String> },
    Picker(Picker),
}

/// A finished piece of background work.
enum Update {
    Scanned {
        generation: u64,
        result: Result<RenamePlan, String>,
    },
    Applied(Result<ApplyResult, PlanChanged>),
}

pub struct App {
    pub directory: TextInput,
    pub recursive: bool,
    pub strict: bool,
    pub level: MatchLevel,
    pub focus: Focus,
    pub tab: Tab,
    pub preview: Option<Preview>,
    pub modal: Option<Modal>,
    pub status: Status,
    pub scanning: bool,
    pub applying: bool,
    /// Advances while work is in flight, to animate the busy indicator.
    pub ticks: usize,
    pub should_quit: bool,
    /// Rejects results from a scan that a newer scan has already replaced.
    generation: u64,
    sender: Sender<Update>,
    receiver: Receiver<Update>,
    /// Clickable regions of the frame just drawn, modal entries last.
    pub hits: Vec<(Rect, Hit)>,
    /// The setup column as last drawn, and how far the wheel has scrolled it.
    ///
    /// A short terminal cannot show the whole column, and the keyboard scrolls
    /// it by moving the focus — which a pointer has no way to do. Without this
    /// the action buttons are simply off the bottom for a mouse-only user.
    pub setup_area: Rect,
    pub setup_scroll: usize,
    /// The focus the setup column was last drawn for, so a focus change can
    /// drag the view back without the wheel being overruled every frame.
    pub setup_focus: Focus,
    /// Where the pointer is, so the draw pass can tint what it is over.
    pub hover: Option<Position>,
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

impl App {
    pub fn new() -> Self {
        let (sender, receiver) = channel();
        Self {
            directory: TextInput::default(),
            recursive: false,
            strict: false,
            level: MatchLevel::default(),
            focus: Focus::Directory,
            tab: Tab::Matched,
            preview: None,
            modal: None,
            status: Status::new(
                "Type a directory path, then press Enter to preview",
                StatusKind::Ready,
            ),
            scanning: false,
            applying: false,
            ticks: 0,
            should_quit: false,
            generation: 0,
            sender,
            receiver,
            hits: Vec::new(),
            setup_area: Rect::ZERO,
            setup_scroll: 0,
            setup_focus: Focus::Directory,
            hover: None,
        }
    }

    /// Start with a directory already filled in, as the command line may supply.
    pub fn with_directory(mut self, directory: &Path) -> Self {
        self.directory.set_value(&directory.to_string_lossy());
        self
    }

    pub fn busy(&self) -> bool {
        self.scanning || self.applying
    }

    /// Whether the apply step is available right now.
    pub fn can_apply(&self) -> bool {
        self.preview
            .as_ref()
            .is_some_and(|preview| !preview.is_demo && !preview.stale && preview.ticked_count() > 0)
            && !self.busy()
    }

    // --------------------------------------------------------------- worker results

    /// Drain anything the worker threads have reported since the last frame.
    pub fn poll_workers(&mut self) {
        while let Ok(update) = self.receiver.try_recv() {
            match update {
                Update::Scanned { generation, result } => {
                    // A newer scan has started; this answer is about a question
                    // nobody is asking any more.
                    if generation != self.generation {
                        continue;
                    }
                    self.scanning = false;
                    match result {
                        Ok(plan) => self.scan_succeeded(plan),
                        Err(error) => {
                            self.status =
                                Status::new(format!("Scan failed: {error}"), StatusKind::Error);
                        }
                    }
                }
                Update::Applied(result) => {
                    self.applying = false;
                    match result {
                        Ok(result) => self.apply_finished(&result),
                        Err(changed) => {
                            self.mark_stale();
                            self.status = Status::new(
                                format!("Files changed on disk — preview again: {changed}"),
                                StatusKind::Error,
                            );
                        }
                    }
                }
            }
        }
    }

    fn scan_succeeded(&mut self, plan: RenamePlan) {
        let (videos, subtitles) = (plan.video_count, plan.subtitle_count);
        self.preview = Some(Preview::new(plan, false));
        self.tab = Tab::Matched;
        self.status = if videos == 0 {
            Status::new("No video files in this directory", StatusKind::Error)
        } else if subtitles == 0 {
            Status::new("No subtitle files in this directory", StatusKind::Error)
        } else {
            Status::new("Preview ready", StatusKind::Success)
        };
        // Move off the path field so the single-letter shortcuts work at once.
        if self
            .preview
            .as_ref()
            .is_some_and(|preview| !preview.prepared.is_empty())
        {
            self.focus = Focus::Results;
        }
    }

    fn apply_finished(&mut self, result: &ApplyResult) {
        // The files just moved, so the preview no longer describes what is on disk.
        self.mark_stale();
        let applied = result.applied.len();
        let failed = result.failed.len();
        self.status = match result.status() {
            ApplyStatus::Completed => Status::new(
                format!("Renamed {applied} {}", plural(applied, "file", "files")),
                StatusKind::Success,
            ),
            ApplyStatus::Partial => Status::new(
                format!(
                    "Renamed {applied}, {failed} failed: {}",
                    first_error(result)
                ),
                StatusKind::Error,
            ),
            ApplyStatus::Failed => Status::new(
                format!("{failed} renames failed: {}", first_error(result)),
                StatusKind::Error,
            ),
        };
    }

    fn mark_stale(&mut self) {
        if let Some(preview) = self.preview.as_mut() {
            preview.stale = true;
        }
    }

    // ---------------------------------------------------------------------- actions

    /// Pending scans and previews only describe the options they were generated from.
    fn invalidate_preview(&mut self) {
        let scan_invalidated = self.scanning;
        if scan_invalidated {
            // The worker cannot be cancelled, but advancing the generation makes
            // its eventual answer harmless and frees the UI to preview again.
            self.generation = self.generation.wrapping_add(1);
            self.scanning = false;
        }

        let preview_invalidated = self.preview.as_mut().is_some_and(|preview| {
            let was_fresh = !preview.stale;
            preview.stale = true;
            was_fresh
        });

        if scan_invalidated || preview_invalidated {
            self.status = Status::new("Options changed — preview again", StatusKind::Working);
        }
    }

    pub fn action_preview(&mut self) {
        if self.busy() {
            return;
        }
        let raw = self.directory.value();
        let raw = raw.trim();
        if raw.is_empty() {
            self.status = Status::new("Enter a directory path first", StatusKind::Error);
            return;
        }
        let directory = crate::paths::resolve(Path::new(raw));
        if !directory.is_dir() {
            self.status = Status::new(format!("Not a directory: {raw}"), StatusKind::Error);
            return;
        }

        self.generation += 1;
        self.scanning = true;
        self.status = Status::new("Scanning…", StatusKind::Working);

        let generation = self.generation;
        let sender = self.sender.clone();
        let options = self.plan_options();
        thread::spawn(move || {
            let result = plan_directory(&directory, &options).map_err(|error| error.to_string());
            let _ = sender.send(Update::Scanned { generation, result });
        });
    }

    pub fn action_demo(&mut self) {
        if self.busy() {
            return;
        }
        self.preview = Some(Preview::new(demo_plan(), true));
        self.tab = Tab::Matched;
        self.focus = Focus::Results;
        self.status = Status::new(
            "Demo mode: sample data, nothing is written",
            StatusKind::Demo,
        );
    }

    pub fn action_apply(&mut self) {
        if self.busy() {
            return;
        }
        let Some(preview) = self.preview.as_ref() else {
            self.status = Status::new("Nothing to apply — preview first", StatusKind::Error);
            return;
        };
        if preview.is_demo {
            self.status = Status::new("Demo mode never writes to disk", StatusKind::Error);
            return;
        }
        if preview.stale {
            self.status = Status::new("Options changed — preview again", StatusKind::Error);
            return;
        }
        let chosen = preview.chosen();
        if chosen.is_empty() {
            self.status = Status::new("Tick at least one subtitle", StatusKind::Error);
            return;
        }

        let root = preview.plan.root.clone();
        let examples = chosen
            .iter()
            .take(CONFIRM_EXAMPLE_LIMIT)
            .map(|prepared| {
                format!(
                    "{}  →  {}",
                    display_path(prepared.source(), &root),
                    file_name(prepared.destination())
                )
            })
            .collect();
        self.modal = Some(Modal::Confirm {
            count: chosen.len(),
            examples,
        });
    }

    fn start_apply(&mut self) {
        let Some(preview) = self.preview.as_ref() else {
            return;
        };
        let chosen = preview.chosen();
        if chosen.is_empty() {
            return;
        }
        let root = preview.plan.root.clone();
        self.applying = true;
        self.status = Status::new("Applying…", StatusKind::Working);

        let sender = self.sender.clone();
        thread::spawn(move || {
            let result = apply_operations(&chosen, &root, false, true);
            let _ = sender.send(Update::Applied(result));
        });
    }

    fn action_browse(&mut self) {
        let typed = self.directory.value();
        let start = if typed.trim().is_empty() {
            std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/"))
        } else {
            let resolved = crate::paths::resolve(Path::new(typed.trim()));
            if resolved.is_dir() {
                resolved
            } else {
                std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/"))
            }
        };
        self.modal = Some(Modal::Picker(Picker::new(&start)));
    }

    fn plan_options(&self) -> PlanOptions {
        PlanOptions {
            recursive: self.recursive,
            strict: self.strict,
            min_score: self.level.score(),
            ..PlanOptions::default()
        }
    }

    // ------------------------------------------------------------------ tick lists

    /// Space on a rename. Says why nothing happened rather than staying silent,
    /// because a key that does nothing reads as a broken key.
    fn toggle_highlighted(&mut self) {
        let tab = self.tab;
        let Some(preview) = self.preview.as_mut() else {
            self.status = Status::new(
                "Nothing to tick yet — p to preview, d for a demo",
                StatusKind::Error,
            );
            return;
        };
        if tab != Tab::Matched {
            self.status = Status::new(
                "Skipped files are not renamed — press ← for the renames",
                StatusKind::Ready,
            );
            return;
        }
        // A preview with nothing in it has no highlighted row to tick, which is
        // the same dead key felt from the other direction.
        let Some(index) = preview.matched_state.selected() else {
            self.status = Status::new(
                "Nothing matched here — try a looser match level, or another folder",
                StatusKind::Ready,
            );
            return;
        };
        if let Some(ticked) = preview.ticked.get_mut(index) {
            *ticked = !*ticked;
        }
    }

    fn tick_all(&mut self, ticked: bool) {
        if let Some(preview) = self.preview.as_mut() {
            preview.ticked.iter_mut().for_each(|item| *item = ticked);
        }
    }

    // ---------------------------------------------------------------- key handling

    pub fn handle_key(&mut self, key: KeyEvent) {
        if key.kind != KeyEventKind::Press {
            return;
        }
        let control = key.modifiers.contains(KeyModifiers::CONTROL);
        if control && matches!(key.code, KeyCode::Char('c')) {
            self.request_quit();
            return;
        }
        if self.applying && matches!(key.code, KeyCode::Char('q')) {
            self.request_quit();
            return;
        }
        if self.modal.is_some() {
            self.handle_modal_key(key);
            return;
        }

        match key.code {
            KeyCode::Tab => self.focus = self.focus.step(1),
            KeyCode::BackTab => self.focus = self.focus.step(-1),
            KeyCode::F(1) => self.modal = Some(Modal::Help),
            // Control keys mean one thing while text is being typed and another
            // over a list, so each context claims its own.
            _ if self.focus == Focus::Directory => self.handle_directory_key(key),
            _ => self.handle_command_key(key),
        }
    }

    /// While the path field has focus, letters have to type rather than act.
    ///
    /// The control keys are the ones a shell prompt answers to, so muscle memory
    /// from the command line carries over.
    fn handle_directory_key(&mut self, key: KeyEvent) {
        let control = key.modifiers.contains(KeyModifiers::CONTROL);
        match key.code {
            KeyCode::Enter => self.action_preview(),
            KeyCode::Esc => self.focus = Focus::Results,
            // The field is the first row of a form, so up and down leave it.
            KeyCode::Up => self.focus = self.focus.step(-1),
            KeyCode::Down => self.focus = self.focus.step(1),
            KeyCode::Char('a') if control => self.directory.move_home(),
            KeyCode::Char('e') if control => self.directory.move_end(),
            KeyCode::Char('u') if control => {
                self.directory.clear();
                self.invalidate_preview();
            }
            KeyCode::Char('k') if control => {
                self.directory.delete_to_end();
                self.invalidate_preview();
            }
            KeyCode::Char('w') if control => {
                self.directory.delete_previous_word();
                self.invalidate_preview();
            }
            // Anything else held with control is a command from elsewhere; it
            // must not end up in the path.
            KeyCode::Char(_) if control => {}
            KeyCode::Char(character) => {
                self.directory.insert(character);
                self.invalidate_preview();
            }
            KeyCode::Backspace if key.modifiers.contains(KeyModifiers::ALT) => {
                self.directory.delete_previous_word();
                self.invalidate_preview();
            }
            KeyCode::Backspace => {
                self.directory.backspace();
                self.invalidate_preview();
            }
            KeyCode::Delete => {
                self.directory.delete();
                self.invalidate_preview();
            }
            KeyCode::Left => self.directory.move_left(),
            KeyCode::Right => self.directory.move_right(),
            KeyCode::Home => self.directory.move_home(),
            KeyCode::End => self.directory.move_end(),
            _ => {}
        }
    }

    /// Everywhere outside the path field, where arrows and their vim twins both
    /// navigate and single letters act.
    fn handle_command_key(&mut self, key: KeyEvent) {
        let control = key.modifiers.contains(KeyModifiers::CONTROL);
        match key.code {
            KeyCode::Char('a') if control => self.tick_all(true),
            KeyCode::Char('r') if control => self.tick_all(false),
            KeyCode::Char('d') if control => self.navigate(HALF_PAGE),
            KeyCode::Char('u') if control => self.navigate(-HALF_PAGE),
            KeyCode::Char('f') if control => self.navigate(PAGE),
            KeyCode::Char('b') if control => self.navigate(-PAGE),
            KeyCode::Char(_) if control => {}
            KeyCode::Char('p') => self.action_preview(),
            KeyCode::Char('a') => self.action_apply(),
            KeyCode::Char('d') => self.action_demo(),
            KeyCode::Char('o') => self.action_browse(),
            // The vim way back into the field that esc leaves.
            KeyCode::Char('i') => self.focus = Focus::Directory,
            KeyCode::Char('?') => self.modal = Some(Modal::Help),
            KeyCode::Char('q') => self.request_quit(),
            // A way back to the path field that never risks quitting by accident.
            KeyCode::Esc => self.focus = Focus::Directory,
            KeyCode::Char(' ') | KeyCode::Enter => self.activate(),
            KeyCode::Up | KeyCode::Char('k') => self.navigate(-1),
            KeyCode::Down | KeyCode::Char('j') => self.navigate(1),
            KeyCode::Left | KeyCode::Char('h') => self.adjust(-1),
            KeyCode::Right | KeyCode::Char('l') => self.adjust(1),
            KeyCode::PageUp => self.navigate(-PAGE),
            KeyCode::PageDown => self.navigate(PAGE),
            KeyCode::Home | KeyCode::Char('g') => self.navigate(isize::MIN),
            KeyCode::End | KeyCode::Char('G') => self.navigate(isize::MAX),
            _ => {}
        }
    }

    /// Space or Enter on whatever currently holds the keyboard.
    fn activate(&mut self) {
        match self.focus {
            Focus::Directory => self.action_preview(),
            Focus::Recursive => {
                self.recursive = !self.recursive;
                self.invalidate_preview();
            }
            Focus::Strict => {
                self.strict = !self.strict;
                self.invalidate_preview();
            }
            Focus::Level => self.set_level(MatchLevel::from_index(
                (self.level.index() + 1) % MatchLevel::ALL.len(),
            )),
            Focus::Results => self.toggle_highlighted(),
        }
    }

    /// Up and down: move within a list, or between the setup controls.
    fn navigate(&mut self, delta: isize) {
        match self.focus {
            Focus::Level => self.step_level(delta),
            Focus::Results => self.move_in_results(delta),
            _ if delta == 1 || delta == -1 => self.focus = self.focus.step(delta.signum()),
            // A switch is one row, so first and last mean the ends of the whole
            // column rather than a nudge to the neighbour.
            _ => {
                self.focus = if delta > 0 {
                    Focus::Results
                } else {
                    Focus::Directory
                }
            }
        }
    }

    /// Walk the match levels, leaving the group at either end.
    ///
    /// Without that the keyboard is trapped: three radios that up and down can
    /// never step out of, with only tab as the way on.
    fn step_level(&mut self, delta: isize) {
        let last = MatchLevel::ALL.len() as isize - 1;
        let target = (self.level.index() as isize).saturating_add(delta);
        // A jump (home, end, a page) stays inside the group; a single step off
        // the end moves to the neighbouring control.
        let single_step = delta == 1 || delta == -1;
        if single_step && !(0..=last).contains(&target) {
            self.focus = self.focus.step(delta.signum());
            return;
        }
        self.set_level(MatchLevel::from_index(target.clamp(0, last) as usize));
    }

    /// Left and right: switch tabs, set a switch, or step through the levels.
    fn adjust(&mut self, delta: isize) {
        match self.focus {
            Focus::Recursive => {
                let value = delta > 0;
                if self.recursive != value {
                    self.recursive = value;
                    self.invalidate_preview();
                }
            }
            Focus::Strict => {
                let value = delta > 0;
                if self.strict != value {
                    self.strict = value;
                    self.invalidate_preview();
                }
            }
            // Sideways within the group only: unlike up and down, this never
            // hands the keyboard to another control.
            Focus::Level => {
                let last = MatchLevel::ALL.len() as isize - 1;
                let target = (self.level.index() as isize + delta.signum()).clamp(0, last);
                self.set_level(MatchLevel::from_index(target as usize));
            }
            Focus::Results => {
                self.tab = if delta > 0 {
                    Tab::Skipped
                } else {
                    Tab::Matched
                };
            }
            Focus::Directory => {}
        }
    }

    fn set_level(&mut self, level: MatchLevel) {
        if self.level == level {
            return;
        }
        self.level = level;
        self.invalidate_preview();
    }

    fn request_quit(&mut self) {
        if self.applying {
            self.status = Status::new(
                "Applying… wait for it to finish before quitting",
                StatusKind::Working,
            );
        } else {
            self.should_quit = true;
        }
    }

    fn move_in_results(&mut self, delta: isize) {
        let tab = self.tab;
        let Some(preview) = self.preview.as_mut() else {
            return;
        };
        match tab {
            Tab::Matched => {
                let count = preview.prepared.len();
                let selected = move_selection(preview.matched_state.selected(), count, delta);
                preview.matched_state.select(selected);
            }
            Tab::Skipped => {
                let count = preview.plan.skipped.len();
                let selected = move_selection(preview.skipped_state.selected(), count, delta);
                preview.skipped_state.select(selected);
            }
        }
    }

    // ---------------------------------------------------------------------- mouse

    /// Movement refreshes the hover point; a left click acts where it lands.
    ///
    /// Capture is always on, so these arrive everywhere including over blank
    /// areas — they simply find no registered rectangle and do nothing.
    pub fn handle_mouse(&mut self, mouse: MouseEvent) {
        let position = Position::new(mouse.column, mouse.row);
        match mouse.kind {
            MouseEventKind::Moved | MouseEventKind::Drag(_) => self.hover = Some(position),
            MouseEventKind::Down(MouseButton::Left) => self.click(position),
            MouseEventKind::ScrollUp => self.scroll(-WHEEL, position),
            MouseEventKind::ScrollDown => self.scroll(WHEEL, position),
            _ => {}
        }
    }

    /// The wheel moves whatever is under it: the picker, the setup column, or
    /// the results.
    fn scroll(&mut self, delta: isize, at: Position) {
        match self.modal.as_mut() {
            Some(Modal::Picker(picker)) => picker.move_by(delta),
            Some(_) => {}
            None if self.setup_area.contains(at) => {
                self.setup_scroll = self.setup_scroll.saturating_add_signed(delta);
            }
            None => self.move_in_results(delta),
        }
    }

    fn click(&mut self, position: Position) {
        self.hover = Some(position);
        // Later registrations sit on top, so a modal covers what is beneath it.
        let Some(hit) = self
            .hits
            .iter()
            .rev()
            .find(|(rect, _)| rect.contains(position))
            .map(|(_, hit)| *hit)
        else {
            return;
        };
        match &self.modal {
            Some(_) => self.click_in_modal(hit),
            None => self.click_main(hit),
        }
    }

    /// Inside a modal only its own controls answer; clicks never fall through.
    fn click_in_modal(&mut self, hit: Hit) {
        match (&mut self.modal, hit) {
            (Some(Modal::Picker(picker)), Hit::PickerRow(index)) => {
                if index < picker.entries.len() {
                    if picker.state.selected() == Some(index) {
                        picker.enter();
                    } else {
                        picker.state.select(Some(index));
                    }
                }
            }
            (Some(Modal::Picker(picker)), Hit::PickerParent) => picker.leave(),
            (Some(Modal::Picker(_)), Hit::PickerUse) => self.use_picker_folder(),
            (Some(Modal::Picker(_)), Hit::PickerCancel) | (Some(Modal::Help), Hit::HelpClose) => {
                self.modal = None;
            }
            (Some(Modal::Confirm { .. }), Hit::ConfirmApply) => {
                self.modal = None;
                self.start_apply();
            }
            (Some(Modal::Confirm { .. }), Hit::ConfirmCancel) => self.modal = None,
            _ => {}
        }
    }

    /// Take the folder the picker is showing as the directory to scan.
    fn use_picker_folder(&mut self) {
        let Some(Modal::Picker(picker)) = &self.modal else {
            return;
        };
        let chosen = picker.current.clone();
        self.modal = None;
        self.directory.set_value(&chosen.to_string_lossy());
        self.invalidate_preview();
        self.focus = Focus::Directory;
    }

    fn click_main(&mut self, hit: Hit) {
        match hit {
            Hit::Directory => self.focus = Focus::Directory,
            Hit::BrowseButton => self.action_browse(),
            Hit::Recursive => {
                self.focus = Focus::Recursive;
                self.recursive = !self.recursive;
                self.invalidate_preview();
            }
            Hit::Strict => {
                self.focus = Focus::Strict;
                self.strict = !self.strict;
                self.invalidate_preview();
            }
            Hit::Level(index) => {
                self.focus = Focus::Level;
                self.set_level(MatchLevel::from_index(index));
            }
            Hit::PreviewButton => self.action_preview(),
            Hit::ApplyButton => self.action_apply(),
            Hit::DemoButton => self.action_demo(),
            Hit::HelpButton => self.modal = Some(Modal::Help),
            Hit::QuitButton => self.request_quit(),
            Hit::TickAll => self.tick_all(true),
            Hit::TickNone => self.tick_all(false),
            Hit::Tab(tab) => {
                self.tab = tab;
                self.focus = Focus::Results;
            }
            Hit::MatchedRow(index) => self.click_matched_row(index),
            Hit::SkippedRow(index) => {
                self.focus = Focus::Results;
                self.tab = Tab::Skipped;
                if let Some(preview) = self.preview.as_mut() {
                    if index < preview.plan.skipped.len() {
                        preview.skipped_state.select(Some(index));
                    }
                }
            }
            // The modals own these variants; reaching one with no modal open
            // means the rectangle outlived the modal, so it is simply ignored.
            Hit::PickerRow(_)
            | Hit::PickerParent
            | Hit::PickerUse
            | Hit::PickerCancel
            | Hit::ConfirmApply
            | Hit::ConfirmCancel
            | Hit::HelpClose => {}
        }
    }

    /// First click selects, second click on the same row ticks — the double
    /// click of every list widget, without timing any double click.
    fn click_matched_row(&mut self, index: usize) {
        let again = self.focus == Focus::Results
            && self.tab == Tab::Matched
            && self
                .preview
                .as_ref()
                .and_then(|preview| preview.matched_state.selected())
                == Some(index);
        self.focus = Focus::Results;
        self.tab = Tab::Matched;
        if let Some(preview) = self.preview.as_mut() {
            if index < preview.prepared.len() {
                preview.matched_state.select(Some(index));
            }
        }
        if again {
            self.toggle_highlighted();
        }
    }

    // -------------------------------------------------------------------- modals

    fn handle_modal_key(&mut self, key: KeyEvent) {
        match self.modal.as_mut() {
            Some(Modal::Help) => {
                if matches!(
                    key.code,
                    KeyCode::Esc | KeyCode::Enter | KeyCode::Char('?' | 'q')
                ) {
                    self.modal = None;
                }
            }
            Some(Modal::Confirm { .. }) => match key.code {
                KeyCode::Enter | KeyCode::Char('y') => {
                    self.modal = None;
                    self.start_apply();
                }
                KeyCode::Esc | KeyCode::Char('n' | 'q') => self.modal = None,
                _ => {}
            },
            Some(Modal::Picker(picker)) => match key.code {
                KeyCode::Up | KeyCode::Char('k') => picker.move_by(-1),
                KeyCode::Down | KeyCode::Char('j') => picker.move_by(1),
                KeyCode::PageUp => picker.move_by(-PAGE),
                KeyCode::PageDown => picker.move_by(PAGE),
                KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    picker.move_by(-HALF_PAGE)
                }
                KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    picker.move_by(HALF_PAGE)
                }
                KeyCode::Home | KeyCode::Char('g') => picker.move_by(isize::MIN),
                KeyCode::End | KeyCode::Char('G') => picker.move_by(isize::MAX),
                KeyCode::Enter | KeyCode::Right | KeyCode::Char('l') => picker.enter(),
                KeyCode::Backspace | KeyCode::Left | KeyCode::Char('h') => picker.leave(),
                KeyCode::Char('s' | ' ') => self.use_picker_folder(),
                KeyCode::Esc | KeyCode::Char('q') => self.modal = None,
                _ => {}
            },
            None => {}
        }
    }
}

/// Move a list selection by `delta`, staying inside `0..count`.
///
/// [`ListState::select_next`] is unbounded, which lets the cursor drift past the
/// end of a list; this keeps it in range.
pub fn move_selection(selected: Option<usize>, count: usize, delta: isize) -> Option<usize> {
    if count == 0 {
        return None;
    }
    let current = selected.unwrap_or(0).min(count - 1) as isize;
    let last = count as isize - 1;
    Some(current.saturating_add(delta).clamp(0, last) as usize)
}

fn first_error(result: &ApplyResult) -> String {
    result
        .failed
        .first()
        .and_then(|outcome| outcome.error.clone())
        .unwrap_or_else(|| "unknown error".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    type ConfigurationChange = (&'static str, fn(&mut App));

    fn app_with_queued_scan() -> App {
        let mut app = App::new();
        app.generation = 7;
        app.scanning = true;
        app.sender
            .send(Update::Scanned {
                generation: app.generation,
                result: Ok(demo_plan()),
            })
            .unwrap();
        app
    }

    fn edit_directory(app: &mut App) {
        app.handle_key(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE));
    }

    fn toggle_recursive(app: &mut App) {
        app.focus = Focus::Recursive;
        app.handle_key(KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE));
    }

    fn toggle_strict(app: &mut App) {
        app.focus = Focus::Strict;
        app.handle_key(KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE));
    }

    fn change_level(app: &mut App) {
        app.focus = Focus::Level;
        app.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE));
    }

    #[test]
    fn configuration_changes_reject_an_in_flight_scan_result() {
        let changes: [ConfigurationChange; 4] = [
            ("directory", edit_directory),
            ("recursive", toggle_recursive),
            ("strict", toggle_strict),
            ("level", change_level),
        ];

        for (name, change) in changes {
            let mut app = app_with_queued_scan();
            change(&mut app);

            assert!(!app.scanning, "{name} left the obsolete scan active");
            app.poll_workers();
            assert!(app.preview.is_none(), "{name} accepted the obsolete plan");
            assert!(!app.can_apply(), "{name} made the obsolete plan applicable");
            assert_eq!(app.status.text, "Options changed — preview again");
        }
    }
}
