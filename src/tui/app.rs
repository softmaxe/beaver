//! State and behaviour of the terminal interface.
//!
//! The screen is a wizard that runs left to right: point at a folder, set the
//! two rules that matter, read a preview that touches nothing, then watch the
//! renames land. One step is on screen at a time, so there is never a question
//! about where to look — the step bar across the top says where you are and the
//! card underneath holds everything that step needs.
//!
//! Scanning and applying both run on a worker thread and report back through a
//! channel, so a large recursive directory never freezes the interface, and the
//! apply step draws a real progress bar rather than an indeterminate spinner.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread;

use ratatui::crossterm::event::{
    KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use ratatui::layout::{Position, Rect};
use ratatui::widgets::ListState;

use crate::applying::{
    apply_operations_reporting, prepare_operations, ApplyResult, ApplyStatus, PlanChanged,
    PreparedOperation,
};
use crate::paths::{display_path, file_name};
use crate::planning::{plan_directory, PlanOptions, RenamePlan};
use crate::presentation::{demo_plan, plural, MatchLevel};
use crate::tui::input::TextInput;
use crate::tui::picker::Picker;

/// How many renames the confirmation dialog spells out before summarising.
const CONFIRM_EXAMPLE_LIMIT: usize = 3;

/// Rows a page key moves through a list, and half of it for `ctrl+d` / `ctrl+u`.
const PAGE: isize = 10;
const HALF_PAGE: isize = 5;

/// Rows one notch of the mouse wheel moves.
const WHEEL: isize = 3;

/// Strict matching is the only matching this interface offers.
///
/// The alternative silently appends a suffix when the target name is taken, and
/// a rename tool that quietly invents a second name is exactly the surprise this
/// redesign is trying to remove. The escape hatch stays on the command line.
const STRICT: bool = true;

// --------------------------------------------------------------------- the wizard

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    Folder,
    Rules,
    Preview,
    Apply,
}

impl Step {
    pub const ORDER: [Self; 4] = [Self::Folder, Self::Rules, Self::Preview, Self::Apply];

    pub fn index(self) -> usize {
        Self::ORDER
            .iter()
            .position(|step| *step == self)
            .unwrap_or(0)
    }

    /// The word under this step's dot in the step bar.
    pub fn label(self) -> &'static str {
        match self {
            Self::Folder => "Folder",
            Self::Rules => "Rules",
            Self::Preview => "Preview",
            Self::Apply => "Apply",
        }
    }

    /// The title on the card, numbered so the bar and the card agree.
    pub fn title(self) -> String {
        format!("{} · {}", self.index() + 1, self.label())
    }
}

/// One focusable thing inside the current step.
///
/// Focus is an index into [`App::controls`] rather than a global enum: each step
/// owns its own short list, which is what keeps a step from having to know about
/// controls that belong to another one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Control {
    /// The path field.
    Path,
    Browse,
    Demo,
    /// The three match levels, which behave as one control.
    Level,
    Recursive,
    /// The list of proposed renames.
    List,
    Back,
    /// The button that moves the wizard one step to the right.
    Advance,
    /// Start over, offered once an apply has finished.
    Again,
}

/// What a left click can land on, registered by the draw pass as rectangles.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hit {
    Control(Control),
    /// One of the three match levels.
    LevelRow(usize),
    /// One proposed rename.
    Row(usize),
    /// A dot in the step bar, for jumping back to a step already visited.
    Dot(usize),
    Help,
    Quit,
    TickAll,
    TickNone,
    Skipped,
    PickerRow(usize),
    PickerParent,
    PickerUse,
    PickerCancel,
    ConfirmApply,
    ConfirmCancel,
    CloseModal,
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

/// A preview: a plan, the checkboxes over it, and whether it is only a sample.
pub struct Preview {
    pub plan: RenamePlan,
    pub prepared: Vec<PreparedOperation>,
    pub ticked: Vec<bool>,
    pub state: ListState,
    pub is_demo: bool,
}

impl Preview {
    fn new(plan: RenamePlan, is_demo: bool) -> Self {
        let prepared = prepare_operations(&plan);
        let mut state = ListState::default();
        if !prepared.is_empty() {
            state.select(Some(0));
        }
        Self {
            ticked: vec![true; prepared.len()],
            prepared,
            plan,
            state,
            is_demo,
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

/// How the apply step ended, which is the whole content of the last card.
pub enum Outcome {
    Done {
        applied: usize,
    },
    Mixed {
        applied: usize,
        failed: usize,
        error: String,
    },
    Refused {
        reason: String,
    },
}

pub enum Modal {
    Help,
    Skipped(ListState),
    Confirm { count: usize, examples: Vec<String> },
    Picker(Picker),
}

/// A finished — or advancing — piece of background work.
enum Update {
    Scanned {
        generation: u64,
        result: Result<RenamePlan, String>,
    },
    Progress(usize),
    Applied(Box<Result<ApplyResult, PlanChanged>>),
}

pub struct App {
    pub step: Step,
    /// Index into [`App::controls`] for the current step.
    pub focus: usize,
    pub directory: TextInput,
    pub recursive: bool,
    pub level: MatchLevel,
    pub preview: Option<Preview>,
    pub outcome: Option<Outcome>,
    /// Renames finished and renames requested, for the progress bar.
    pub progress: (usize, usize),
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
            step: Step::Folder,
            focus: 0,
            directory: TextInput::default(),
            recursive: false,
            level: MatchLevel::default(),
            preview: None,
            outcome: None,
            progress: (0, 0),
            modal: None,
            status: Status::new("Type or browse to a folder", StatusKind::Ready),
            scanning: false,
            applying: false,
            ticks: 0,
            should_quit: false,
            generation: 0,
            sender,
            receiver,
            hits: Vec::new(),
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

    /// The focusable controls of the step on screen, in top-to-bottom order.
    pub fn controls(&self) -> Vec<Control> {
        match self.step {
            Step::Folder => vec![
                Control::Path,
                Control::Browse,
                Control::Demo,
                Control::Advance,
            ],
            Step::Rules => vec![
                Control::Level,
                Control::Recursive,
                Control::Back,
                Control::Advance,
            ],
            Step::Preview if self.scanning => Vec::new(),
            Step::Preview => vec![Control::List, Control::Back, Control::Advance],
            Step::Apply if self.applying => Vec::new(),
            Step::Apply => vec![Control::Again],
        }
    }

    /// Which control holds the keyboard, if the step has any.
    pub fn control(&self) -> Option<Control> {
        self.controls().get(self.focus).copied()
    }

    pub fn is_focused(&self, control: Control) -> bool {
        self.control() == Some(control)
    }

    /// Whether the apply step is available right now.
    pub fn can_apply(&self) -> bool {
        self.step == Step::Preview
            && !self.busy()
            && self
                .preview
                .as_ref()
                .is_some_and(|preview| !preview.is_demo && preview.ticked_count() > 0)
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
                            self.step = Step::Rules;
                            self.focus_on(Control::Advance);
                            self.status =
                                Status::new(format!("Scan failed: {error}"), StatusKind::Error);
                        }
                    }
                }
                Update::Progress(done) => self.progress.0 = done,
                Update::Applied(result) => {
                    self.applying = false;
                    match *result {
                        Ok(result) => self.apply_finished(&result),
                        Err(changed) => {
                            self.preview = None;
                            self.outcome = Some(Outcome::Refused {
                                reason: changed.to_string(),
                            });
                            self.status = Status::new(
                                "Files moved under the preview — nothing was renamed",
                                StatusKind::Error,
                            );
                        }
                    }
                    self.focus = 0;
                }
            }
        }
    }

    fn scan_succeeded(&mut self, plan: RenamePlan) {
        let (videos, subtitles, matched) =
            (plan.video_count, plan.subtitle_count, plan.operations.len());
        self.preview = Some(Preview::new(plan, false));
        self.step = Step::Preview;
        self.focus = 0;
        self.status = if videos == 0 {
            Status::new("No video files in this folder", StatusKind::Error)
        } else if subtitles == 0 {
            Status::new("No subtitle files in this folder", StatusKind::Error)
        } else if matched == 0 {
            Status::new(
                "Nothing matched — try a looser match level",
                StatusKind::Error,
            )
        } else {
            Status::new(
                "Preview ready — nothing has been written",
                StatusKind::Ready,
            )
        };
    }

    fn apply_finished(&mut self, result: &ApplyResult) {
        // The files just moved, so the preview no longer describes what is on disk.
        self.preview = None;
        let applied = result.applied.len();
        let failed = result.failed.len();
        self.progress = (applied + failed, applied + failed);
        match result.status() {
            ApplyStatus::Completed => {
                self.outcome = Some(Outcome::Done { applied });
                self.status = Status::new(
                    format!("Renamed {applied} {}", plural(applied, "file", "files")),
                    StatusKind::Success,
                );
            }
            ApplyStatus::Partial | ApplyStatus::Failed => {
                let error = first_error(result);
                self.status = Status::new(
                    format!("{failed} {} failed", plural(failed, "rename", "renames")),
                    StatusKind::Error,
                );
                self.outcome = Some(Outcome::Mixed {
                    applied,
                    failed,
                    error,
                });
            }
        }
    }

    // ---------------------------------------------------------------------- actions

    /// A changed rule makes any preview a description of a question nobody asked.
    fn invalidate_preview(&mut self) {
        if self.scanning {
            // The worker cannot be cancelled, but advancing the generation makes
            // its eventual answer harmless.
            self.generation = self.generation.wrapping_add(1);
            self.scanning = false;
        }
        self.preview = None;
    }

    /// The button on the right of every card: move one step along.
    pub fn advance(&mut self) {
        match self.step {
            Step::Folder => self.leave_folder(),
            Step::Rules => self.action_preview(),
            Step::Preview => self.action_apply(),
            Step::Apply => self.start_over(),
        }
    }

    /// The button on the left of every card after the first: move one step back.
    pub fn back(&mut self) {
        match self.step {
            Step::Folder => {}
            Step::Rules => self.go_to(Step::Folder),
            Step::Preview if self.scanning => {
                self.invalidate_preview();
                self.go_to(Step::Rules);
            }
            Step::Preview => self.go_to(Step::Rules),
            Step::Apply if self.applying => {}
            Step::Apply => self.start_over(),
        }
    }

    fn go_to(&mut self, step: Step) {
        self.step = step;
        self.focus = 0;
        if step == Step::Folder {
            self.status = Status::new("Type or browse to a folder", StatusKind::Ready);
        }
    }

    fn start_over(&mut self) {
        self.outcome = None;
        self.progress = (0, 0);
        self.preview = None;
        self.go_to(Step::Folder);
    }

    /// Leave the folder step, but only for a path that actually exists.
    fn leave_folder(&mut self) {
        match self.resolved_directory() {
            Ok(_) => {
                self.go_to(Step::Rules);
                self.status = Status::new("Two rules, then a preview", StatusKind::Ready);
            }
            Err(message) => self.status = Status::new(message, StatusKind::Error),
        }
    }

    fn resolved_directory(&self) -> Result<PathBuf, String> {
        let raw = self.directory.value().trim().to_string();
        if raw.is_empty() {
            return Err("Enter a folder path first".into());
        }
        let directory = crate::paths::resolve(Path::new(&raw));
        if !directory.is_dir() {
            return Err(format!("Not a folder: {raw}"));
        }
        Ok(directory)
    }

    /// Scan the chosen folder, showing the preview step while the worker runs.
    pub fn action_preview(&mut self) {
        if self.busy() {
            return;
        }
        let directory = match self.resolved_directory() {
            Ok(directory) => directory,
            Err(message) => {
                self.go_to(Step::Folder);
                self.status = Status::new(message, StatusKind::Error);
                return;
            }
        };

        self.generation += 1;
        self.scanning = true;
        self.preview = None;
        self.outcome = None;
        self.step = Step::Preview;
        self.focus = 0;
        self.status = Status::new("Scanning…", StatusKind::Working);

        let generation = self.generation;
        let sender = self.sender.clone();
        let options = self.plan_options();
        thread::spawn(move || {
            let result = plan_directory(&directory, &options).map_err(|error| error.to_string());
            let _ = sender.send(Update::Scanned { generation, result });
        });
    }

    /// Load the sample library, which looks real and writes nothing.
    pub fn action_demo(&mut self) {
        if self.busy() {
            return;
        }
        self.invalidate_preview();
        self.outcome = None;
        self.preview = Some(Preview::new(demo_plan(), true));
        self.step = Step::Preview;
        self.focus = 0;
        self.status = Status::new("Demo — sample data, nothing is written", StatusKind::Demo);
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
            self.status = Status::new("Demo mode never writes to disk", StatusKind::Demo);
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
        self.progress = (0, chosen.len());
        self.step = Step::Apply;
        self.focus = 0;
        self.status = Status::new("Renaming…", StatusKind::Working);

        let sender = self.sender.clone();
        let progress = self.sender.clone();
        thread::spawn(move || {
            let result = apply_operations_reporting(&chosen, &root, false, true, |done| {
                let _ = progress.send(Update::Progress(done));
            });
            let _ = sender.send(Update::Applied(Box::new(result)));
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

    fn show_skipped(&mut self) {
        let count = self
            .preview
            .as_ref()
            .map(|preview| preview.plan.skipped.len())
            .unwrap_or(0);
        if count == 0 {
            return;
        }
        let mut state = ListState::default();
        state.select(Some(0));
        self.modal = Some(Modal::Skipped(state));
    }

    fn plan_options(&self) -> PlanOptions {
        PlanOptions {
            recursive: self.recursive,
            strict: STRICT,
            min_score: self.level.score(),
            ..PlanOptions::default()
        }
    }

    // ------------------------------------------------------------------ tick lists

    /// Space on a rename. Says why nothing happened rather than staying silent,
    /// because a key that does nothing reads as a broken key.
    fn toggle_highlighted(&mut self) {
        let Some(preview) = self.preview.as_mut() else {
            return;
        };
        let Some(index) = preview.state.selected() else {
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
        if self.modal.is_some() {
            self.handle_modal_key(key);
            return;
        }

        match key.code {
            KeyCode::Tab => self.move_focus(1),
            KeyCode::BackTab => self.move_focus(-1),
            KeyCode::F(1) => self.modal = Some(Modal::Help),
            // Letters have to type rather than act while the path field is live,
            // so the field claims every key it can possibly mean.
            _ if self.is_focused(Control::Path) => self.handle_path_key(key),
            _ => self.handle_command_key(key),
        }
    }

    /// While the path field has focus, the control keys are the ones a shell
    /// prompt answers to, so muscle memory from the command line carries over.
    fn handle_path_key(&mut self, key: KeyEvent) {
        let control = key.modifiers.contains(KeyModifiers::CONTROL);
        match key.code {
            KeyCode::Enter => self.advance(),
            KeyCode::Esc | KeyCode::Down => self.move_focus(1),
            KeyCode::Up => self.move_focus(-1),
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

    /// Everywhere outside the path field.
    ///
    /// Left and right are the workflow: they walk the wizard the way the step bar
    /// is drawn. Up and down stay inside the card. Nothing means two things.
    fn handle_command_key(&mut self, key: KeyEvent) {
        let control = key.modifiers.contains(KeyModifiers::CONTROL);
        match key.code {
            KeyCode::Char('a') if control => self.tick_all(true),
            KeyCode::Char('r') if control => self.tick_all(false),
            KeyCode::Char('d') if control => self.move_in_list(HALF_PAGE),
            KeyCode::Char('u') if control => self.move_in_list(-HALF_PAGE),
            KeyCode::Char(_) if control => {}
            KeyCode::Char('p') => self.action_preview(),
            KeyCode::Char('a') => self.action_apply(),
            KeyCode::Char('d') => self.action_demo(),
            KeyCode::Char('o') => self.action_browse(),
            KeyCode::Char('s') => self.show_skipped(),
            KeyCode::Char('i') => self.focus_on(Control::Path),
            KeyCode::Char('?') => self.modal = Some(Modal::Help),
            KeyCode::Char('q') => self.request_quit(),
            KeyCode::Esc => self.back(),
            // Enter always moves the wizard on, whatever holds the keyboard;
            // space presses the focused control in place. A key that means
            // "forward" everywhere is worth more than one that means five things.
            KeyCode::Enter => self.advance(),
            KeyCode::Char(' ') => self.activate(),
            KeyCode::Left | KeyCode::Char('h') => self.back(),
            KeyCode::Right | KeyCode::Char('l') => self.advance(),
            KeyCode::Up | KeyCode::Char('k') => self.step_within(-1),
            KeyCode::Down | KeyCode::Char('j') => self.step_within(1),
            KeyCode::PageUp => self.move_in_list(-PAGE),
            KeyCode::PageDown => self.move_in_list(PAGE),
            KeyCode::Home | KeyCode::Char('g') => self.move_in_list(isize::MIN),
            KeyCode::End | KeyCode::Char('G') => self.move_in_list(isize::MAX),
            _ => {}
        }
    }

    /// Enter or space on whatever currently holds the keyboard.
    fn activate(&mut self) {
        match self.control() {
            Some(Control::Path) => self.advance(),
            Some(Control::Browse) => self.action_browse(),
            Some(Control::Demo) => self.action_demo(),
            Some(Control::Level) => self.set_level(MatchLevel::from_index(
                (self.level.index() + 1) % MatchLevel::ALL.len(),
            )),
            Some(Control::Recursive) => {
                self.recursive = !self.recursive;
                self.invalidate_preview();
            }
            Some(Control::List) => self.toggle_highlighted(),
            Some(Control::Back) => self.back(),
            Some(Control::Advance) | Some(Control::Again) => self.advance(),
            None => {}
        }
    }

    /// Up and down inside the card: through the list, the levels, or the controls.
    ///
    /// A group that up and down can never step out of is a trap, so the levels
    /// and the list both hand the keyboard on at their edges.
    fn step_within(&mut self, delta: isize) {
        match self.control() {
            Some(Control::Level) => {
                let last = MatchLevel::ALL.len() as isize - 1;
                let target = self.level.index() as isize + delta;
                if (0..=last).contains(&target) {
                    self.set_level(MatchLevel::from_index(target as usize));
                } else {
                    self.move_focus(delta);
                }
            }
            Some(Control::List) => {
                let at_edge = self
                    .preview
                    .as_ref()
                    .map(|preview| {
                        let count = preview.prepared.len();
                        match preview.state.selected() {
                            None => true,
                            Some(index) if delta < 0 => index == 0,
                            Some(index) => index + 1 >= count,
                        }
                    })
                    .unwrap_or(true);
                if at_edge && delta > 0 {
                    self.move_focus(delta);
                } else {
                    self.move_in_list(delta);
                }
            }
            _ => self.move_focus(delta),
        }
    }

    fn move_focus(&mut self, delta: isize) {
        let count = self.controls().len() as isize;
        if count == 0 {
            return;
        }
        self.focus = (self.focus as isize + delta).rem_euclid(count) as usize;
    }

    fn focus_on(&mut self, control: Control) {
        if let Some(index) = self.controls().iter().position(|item| *item == control) {
            self.focus = index;
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
                "Renaming… wait for it to finish before quitting",
                StatusKind::Working,
            );
        } else {
            self.should_quit = true;
        }
    }

    fn move_in_list(&mut self, delta: isize) {
        let Some(preview) = self.preview.as_mut() else {
            return;
        };
        let count = preview.prepared.len();
        let selected = move_selection(preview.state.selected(), count, delta);
        preview.state.select(selected);
    }

    // ---------------------------------------------------------------------- mouse

    /// Movement refreshes the hover point; a left click acts where it lands.
    pub fn handle_mouse(&mut self, mouse: MouseEvent) {
        let position = Position::new(mouse.column, mouse.row);
        match mouse.kind {
            MouseEventKind::Moved | MouseEventKind::Drag(_) => self.hover = Some(position),
            MouseEventKind::Down(MouseButton::Left) => self.click(position),
            MouseEventKind::ScrollUp => self.scroll(-WHEEL),
            MouseEventKind::ScrollDown => self.scroll(WHEEL),
            _ => {}
        }
    }

    /// The wheel moves whichever list is on top.
    fn scroll(&mut self, delta: isize) {
        match self.modal.as_mut() {
            Some(Modal::Picker(picker)) => picker.move_by(delta),
            Some(Modal::Skipped(state)) => {
                let count = self
                    .preview
                    .as_ref()
                    .map(|preview| preview.plan.skipped.len())
                    .unwrap_or(0);
                let selected = move_selection(state.selected(), count, delta);
                state.select(selected);
            }
            Some(_) => {}
            None => self.move_in_list(delta),
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
            (Some(Modal::Picker(_)), Hit::PickerCancel)
            | (Some(Modal::Help), Hit::CloseModal)
            | (Some(Modal::Skipped(_)), Hit::CloseModal) => self.modal = None,
            (Some(Modal::Skipped(state)), Hit::Row(index)) => state.select(Some(index)),
            (Some(Modal::Confirm { .. }), Hit::ConfirmApply) => {
                self.modal = None;
                self.start_apply();
            }
            (Some(Modal::Confirm { .. }), Hit::ConfirmCancel) => self.modal = None,
            _ => {}
        }
    }

    /// Take the folder the picker is showing as the folder to scan.
    fn use_picker_folder(&mut self) {
        let Some(Modal::Picker(picker)) = &self.modal else {
            return;
        };
        let chosen = picker.current.clone();
        self.modal = None;
        self.directory.set_value(&chosen.to_string_lossy());
        self.invalidate_preview();
        self.go_to(Step::Folder);
    }

    fn click_main(&mut self, hit: Hit) {
        match hit {
            Hit::Control(control) => self.click_control(control),
            Hit::LevelRow(index) => {
                self.focus_on(Control::Level);
                self.set_level(MatchLevel::from_index(index));
            }
            Hit::Row(index) => self.click_row(index),
            Hit::Dot(index) => self.click_dot(index),
            Hit::Help => self.modal = Some(Modal::Help),
            Hit::Quit => self.request_quit(),
            Hit::TickAll => self.tick_all(true),
            Hit::TickNone => self.tick_all(false),
            Hit::Skipped => self.show_skipped(),
            // The modals own these variants; reaching one with no modal open
            // means the rectangle outlived the modal, so it is simply ignored.
            Hit::PickerRow(_)
            | Hit::PickerParent
            | Hit::PickerUse
            | Hit::PickerCancel
            | Hit::ConfirmApply
            | Hit::ConfirmCancel
            | Hit::CloseModal => {}
        }
    }

    fn click_control(&mut self, control: Control) {
        self.focus_on(control);
        match control {
            // The field only takes focus; clicking it must not submit it.
            Control::Path => {}
            Control::Browse => self.action_browse(),
            Control::Demo => self.action_demo(),
            Control::Recursive => {
                self.recursive = !self.recursive;
                self.invalidate_preview();
            }
            Control::Level | Control::List => {}
            Control::Back => self.back(),
            Control::Advance | Control::Again => self.advance(),
        }
    }

    /// First click selects, second click on the same row ticks — the double
    /// click of every list widget, without timing any double click.
    fn click_row(&mut self, index: usize) {
        let again = self.is_focused(Control::List)
            && self
                .preview
                .as_ref()
                .and_then(|preview| preview.state.selected())
                == Some(index);
        self.focus_on(Control::List);
        if let Some(preview) = self.preview.as_mut() {
            if index < preview.prepared.len() {
                preview.state.select(Some(index));
            }
        }
        if again {
            self.toggle_highlighted();
        }
    }

    /// A dot in the step bar walks back to a step already visited.
    ///
    /// Forward is deliberately not clickable: moving on has preconditions, and a
    /// dot that silently refuses reads as broken. `Advance` is the way forward.
    fn click_dot(&mut self, index: usize) {
        if self.busy() || index >= self.step.index() {
            return;
        }
        match Step::ORDER[index] {
            Step::Folder => self.go_to(Step::Folder),
            Step::Rules => self.go_to(Step::Rules),
            _ => {}
        }
    }

    // -------------------------------------------------------------------- modals

    fn handle_modal_key(&mut self, key: KeyEvent) {
        let skipped_count = self
            .preview
            .as_ref()
            .map(|preview| preview.plan.skipped.len())
            .unwrap_or(0);
        match self.modal.as_mut() {
            Some(Modal::Help) => {
                if matches!(
                    key.code,
                    KeyCode::Esc | KeyCode::Enter | KeyCode::Char('?' | 'q' | ' ')
                ) {
                    self.modal = None;
                }
            }
            Some(Modal::Skipped(state)) => match key.code {
                KeyCode::Up | KeyCode::Char('k') => {
                    state.select(move_selection(state.selected(), skipped_count, -1))
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    state.select(move_selection(state.selected(), skipped_count, 1))
                }
                KeyCode::Home | KeyCode::Char('g') => {
                    state.select(move_selection(state.selected(), skipped_count, isize::MIN))
                }
                KeyCode::End | KeyCode::Char('G') => {
                    state.select(move_selection(state.selected(), skipped_count, isize::MAX))
                }
                KeyCode::Esc | KeyCode::Enter | KeyCode::Char('s' | 'q' | ' ') => self.modal = None,
                _ => {}
            },
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

    type RuleChange = (&'static str, fn(&mut App));

    fn app_with_queued_scan() -> App {
        let mut app = App::new();
        app.step = Step::Rules;
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
        app.step = Step::Folder;
        app.focus = 0;
        app.handle_key(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE));
    }

    fn toggle_recursive(app: &mut App) {
        app.focus_on(Control::Recursive);
        app.handle_key(KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE));
    }

    fn change_level(app: &mut App) {
        app.focus_on(Control::Level);
        app.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
    }

    #[test]
    fn rule_changes_reject_an_in_flight_scan_result() {
        let changes: [RuleChange; 3] = [
            ("directory", edit_directory),
            ("recursive", toggle_recursive),
            ("level", change_level),
        ];

        for (name, change) in changes {
            let mut app = app_with_queued_scan();
            change(&mut app);

            assert!(!app.scanning, "{name} left the obsolete scan active");
            app.poll_workers();
            assert!(app.preview.is_none(), "{name} accepted the obsolete plan");
            assert!(!app.can_apply(), "{name} made the obsolete plan applicable");
        }
    }

    #[test]
    fn the_wizard_walks_both_ways() {
        let mut app = App::new();
        assert_eq!(app.step, Step::Folder);
        // An empty path is not a folder, so the first step refuses to be left.
        app.advance();
        assert_eq!(app.step, Step::Folder);
        assert_eq!(app.status.kind, StatusKind::Error);

        app.directory.set_value(".");
        app.advance();
        assert_eq!(app.step, Step::Rules);
        app.back();
        assert_eq!(app.step, Step::Folder);
    }

    #[test]
    fn a_demo_jumps_to_the_preview_and_can_never_be_applied() {
        let mut app = App::new();
        app.action_demo();
        assert_eq!(app.step, Step::Preview);
        assert!(!app.can_apply());
        app.advance();
        assert!(app.modal.is_none());
        assert_eq!(app.status.kind, StatusKind::Demo);
    }
}
