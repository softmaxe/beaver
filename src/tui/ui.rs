//! Drawing the interface.
//!
//! Layout is measured in character cells, not pixels. Everything the user
//! configures lives on the left, everything the tool reports lives on the right;
//! that split is what makes the three-step workflow legible without numbering the
//! steps. Below 100 columns the two panes cannot sit side by side, so they stack.

use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{
    Block, BorderType, Cell, Clear, List, ListItem, Padding, Paragraph, Row, Table, Wrap,
};
use ratatui::Frame;
use unicode_width::UnicodeWidthStr;

use crate::applying::PreparedOperation;
use crate::paths::display_path;
use crate::planning::{MatchReason, RenamePlan};
use crate::presentation::{match_badge, skip_label, MatchLevel};
use crate::tui::app::{App, Focus, Modal, StatusKind, Tab};
use crate::tui::theme;

/// Narrower than this, the setup column and the results cannot share a row.
const WIDE_LAYOUT_MINIMUM: u16 = 100;
const SETUP_WIDTH: u16 = 38;
const SPINNER: [&str; 4] = ["⠋", "⠙", "⠹", "⠸"];

pub fn draw(frame: &mut Frame, app: &mut App) {
    let area = frame.area();
    frame.render_widget(Block::new().style(theme::base()), area);

    let [header_area, body_area, footer_area] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(3),
        Constraint::Length(1),
    ])
    .areas(area);

    draw_header(frame, header_area);
    if body_area.width >= WIDE_LAYOUT_MINIMUM {
        let [setup_area, results_area] =
            Layout::horizontal([Constraint::Length(SETUP_WIDTH), Constraint::Min(20)])
                .areas(body_area);
        draw_setup(frame, app, setup_area);
        draw_results(frame, app, results_area);
    } else {
        // Setup keeps at most 45% of the height and scrolls internally, so the
        // results stay on screen even in an 80x24 terminal.
        let setup_height = (body_area.height * 45 / 100).max(6);
        let [setup_area, results_area] =
            Layout::vertical([Constraint::Length(setup_height), Constraint::Min(5)])
                .areas(body_area);
        draw_setup(frame, app, setup_area);
        draw_results(frame, app, results_area);
    }
    draw_footer(frame, app, footer_area);

    match app.modal.as_mut() {
        Some(Modal::Help) => draw_help(frame, area),
        Some(Modal::Confirm { count, examples }) => draw_confirm(frame, area, *count, examples),
        Some(Modal::Picker(picker)) => draw_picker(frame, area, picker),
        None => {}
    }
}

// ------------------------------------------------------------------ chrome

fn draw_header(frame: &mut Frame, area: Rect) {
    let line = Line::from(vec![
        Span::styled(" Subtitle Renamer ", theme::heading()),
        Span::styled(
            "· align subtitle filenames with the videos beside them",
            theme::faint(),
        ),
    ]);
    frame.render_widget(
        Paragraph::new(line).style(Style::default().bg(theme::PANEL)),
        area,
    );
}

/// The keys the focused control answers to, then the two that always apply.
///
/// A fixed list would have to be either too long to read or too short to help;
/// showing what is live right now is what makes ticking and toggling findable
/// without opening the help.
fn draw_footer(frame: &mut Frame, app: &App, area: Rect) {
    let apply_style = if app.can_apply() {
        theme::key()
    } else {
        theme::faint()
    };
    let mut hints: Vec<(&str, &str, Style)> = match app.focus {
        Focus::Directory => vec![
            ("enter", "preview", theme::key()),
            ("o", "browse", theme::key()),
            ("↓", "next field", theme::key()),
            ("esc", "leave field", theme::key()),
        ],
        Focus::Recursive | Focus::Strict => vec![
            ("space", "toggle", theme::key()),
            ("←→", "set", theme::key()),
            ("↑↓", "move", theme::key()),
            ("p", "preview", theme::key()),
        ],
        Focus::Level => vec![
            ("↑↓", "choose", theme::key()),
            ("space", "cycle", theme::key()),
            ("p", "preview", theme::key()),
        ],
        Focus::Results => vec![
            ("space", "tick", theme::key()),
            ("a", "apply", apply_style),
            ("p", "preview", theme::key()),
            ("←→", "tabs", theme::key()),
            ("^a", "all", theme::key()),
            ("^r", "none", theme::key()),
            ("d", "demo", theme::key()),
        ],
    };
    hints.push(("?", "help", theme::key()));
    hints.push(("q", "quit", theme::key()));

    // Help and quit are the two that survive a narrow terminal; the rest drop
    // off the end until the line fits.
    const ALWAYS_KEPT: usize = 2;
    while hints.len() > ALWAYS_KEPT && hints_width(&hints) > area.width as usize {
        hints.remove(hints.len() - ALWAYS_KEPT - 1);
    }

    let mut spans = vec![Span::raw(" ")];
    for (index, (key, label, style)) in hints.into_iter().enumerate() {
        if index > 0 {
            spans.push(Span::styled(" · ", theme::faint()));
        }
        spans.push(Span::styled(key, style));
        spans.push(Span::styled(format!(" {label}"), theme::muted()));
    }
    frame.render_widget(
        Paragraph::new(Line::from(spans)).style(Style::default().bg(theme::PANEL)),
        area,
    );
}

/// Columns the footer takes: a leading space, the pairs, and " · " between them.
fn hints_width(hints: &[(&str, &str, Style)]) -> usize {
    let pairs: usize = hints
        .iter()
        .map(|(key, label, _)| key.width() + 1 + label.width())
        .sum();
    1 + pairs + 3 * hints.len().saturating_sub(1)
}

// ------------------------------------------------------------- setup column

fn draw_setup(frame: &mut Frame, app: &mut App, area: Rect) {
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(theme::border(false))
        .style(Style::default().bg(theme::SURFACE))
        .title_top(Span::styled(" Setup ", theme::heading()));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.height == 0 || inner.width == 0 {
        return;
    }

    let width = inner.width as usize;
    let mut rows: Vec<SetupRow> = Vec::new();

    rows.push(SetupRow::text(label_line(
        "Directory",
        app.focus == Focus::Directory,
        "enter",
        width,
    )));
    let (text, cursor) = app.directory.view(width.saturating_sub(3));
    rows.push(SetupRow::input(
        &text,
        app.directory.is_empty(),
        app.focus == Focus::Directory,
    ));
    let input_row = rows.len() - 1;
    rows.push(SetupRow::text(hint_line("o  browse for a folder")));
    rows.push(SetupRow::blank());

    rows.push(SetupRow::text(label_line("Options", false, "", width)));
    let recursive_row = rows.len();
    rows.push(SetupRow::switch(
        app.recursive,
        "Include subfolders",
        app.focus == Focus::Recursive,
        width,
    ));
    let strict_row = rows.len();
    rows.push(SetupRow::switch(
        app.strict,
        "Strict mode",
        app.focus == Focus::Strict,
        width,
    ));
    for line in wrap(
        "Only accept subtitles that fit VideoName+ext exactly",
        width,
    ) {
        rows.push(SetupRow::text(hint_line(&line)));
    }
    rows.push(SetupRow::blank());

    rows.push(SetupRow::text(label_line(
        "Match level",
        app.focus == Focus::Level,
        "↑↓",
        width,
    )));
    let mut level_rows = Vec::new();
    for level in MatchLevel::ALL {
        level_rows.push(rows.len());
        rows.push(SetupRow::radio(
            level == app.level,
            level.label(),
            app.focus == Focus::Level,
            width,
        ));
    }
    for line in wrap(app.level.hint(), width) {
        rows.push(SetupRow::text(hint_line(&line)));
    }
    rows.push(SetupRow::blank());

    rows.push(SetupRow::button("p", "Preview", ButtonKind::Primary, width));
    rows.push(SetupRow::button(
        "a",
        "Apply renames",
        if app.can_apply() {
            ButtonKind::Confirm
        } else {
            ButtonKind::Disabled
        },
        width,
    ));
    rows.push(SetupRow::button(
        "d",
        "Demo mode",
        ButtonKind::Neutral,
        width,
    ));

    // Scroll just enough to keep whatever holds the keyboard on screen.
    let focus_row = match app.focus {
        Focus::Directory => input_row,
        Focus::Recursive => recursive_row,
        Focus::Strict => strict_row,
        Focus::Level => level_rows[app.level.index()],
        Focus::Results => 0,
    };
    let height = inner.height as usize;
    let scroll = if rows.len() <= height {
        0
    } else {
        focus_row
            .saturating_sub(height.saturating_sub(1))
            .min(rows.len() - height)
    };

    for (offset, row) in rows.iter().enumerate().skip(scroll).take(height) {
        let y = inner.y + (offset - scroll) as u16;
        let line_area = Rect::new(inner.x, y, inner.width, 1);
        frame.render_widget(Paragraph::new(row.line.clone()).style(row.style), line_area);
        if offset == input_row && app.focus == Focus::Directory {
            let column = inner.x + 2 + cursor as u16;
            frame.set_cursor_position((column.min(inner.right().saturating_sub(1)), y));
        }
    }
}

/// One rendered line of the setup column.
struct SetupRow {
    line: Line<'static>,
    style: Style,
}

enum ButtonKind {
    Primary,
    Confirm,
    Neutral,
    Disabled,
}

impl SetupRow {
    fn text(line: Line<'static>) -> Self {
        Self {
            line,
            style: Style::default().bg(theme::SURFACE),
        }
    }

    fn blank() -> Self {
        Self::text(Line::default())
    }

    fn input(text: &str, empty: bool, focused: bool) -> Self {
        let content = if empty && !focused {
            Span::styled("Type or paste a directory path", theme::faint())
        } else {
            Span::styled(text.to_string(), Style::default().fg(theme::FOREGROUND))
        };
        Self {
            line: Line::from(vec![
                Span::styled(caret(focused), Style::default().fg(theme::FOCUS)),
                content,
            ]),
            style: Style::default().bg(if focused {
                theme::SELECTION_BACKGROUND
            } else {
                theme::PANEL
            }),
        }
    }

    fn switch(on: bool, label: &str, focused: bool, width: usize) -> Self {
        let marker = if on {
            Span::styled("[✓] ", Style::default().fg(theme::TICK))
        } else {
            Span::styled("[ ] ", theme::faint())
        };
        Self::control(marker, label, focused, "space", width)
    }

    fn radio(selected: bool, label: &str, focused: bool, width: usize) -> Self {
        let marker = if selected {
            Span::styled("(●) ", Style::default().fg(theme::FOCUS))
        } else {
            Span::styled("( ) ", theme::faint())
        };
        Self::control(marker, label, focused && selected, "", width)
    }

    /// A checkbox or radio row: caret, marker, label, and — while it holds the
    /// keyboard — the key that changes it, spelled out at the end of the row.
    fn control(
        marker: Span<'static>,
        label: &str,
        highlighted: bool,
        hint: &str,
        width: usize,
    ) -> Self {
        let label_style = if highlighted {
            Style::default()
                .fg(theme::FOREGROUND)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme::MUTED)
        };
        let hint = if highlighted { hint } else { "" };
        let room = width.saturating_sub(CARET.width() + 4);
        let mut spans = vec![
            Span::styled(caret(highlighted), Style::default().fg(theme::FOCUS)),
            marker,
            Span::styled(pad(label, room.saturating_sub(hint.width())), label_style),
        ];
        if !hint.is_empty() {
            spans.push(Span::styled(hint.to_string(), theme::faint()));
        }
        Self {
            line: Line::from(spans),
            style: Style::default().bg(if highlighted {
                theme::SELECTION_BACKGROUND
            } else {
                theme::SURFACE
            }),
        }
    }

    fn button(key: &str, label: &str, kind: ButtonKind, width: usize) -> Self {
        let (background, foreground) = match kind {
            ButtonKind::Primary => (theme::FOCUS, theme::PANEL),
            ButtonKind::Confirm => (theme::SUCCESS, theme::PANEL),
            ButtonKind::Neutral => (theme::SELECTION_BACKGROUND, theme::FOREGROUND),
            ButtonKind::Disabled => (theme::SELECTION_BACKGROUND, theme::FAINT),
        };
        let text = format!("{key}  {label}");
        let inner_width = width.saturating_sub(2);
        let left = inner_width.saturating_sub(text.width()) / 2;
        let content = format!(
            " {}{}{} ",
            " ".repeat(left),
            text,
            " ".repeat(inner_width.saturating_sub(left + text.width()))
        );
        Self {
            line: Line::from(Span::styled(
                content,
                Style::default()
                    .fg(foreground)
                    .bg(background)
                    .add_modifier(Modifier::BOLD),
            )),
            style: Style::default().bg(theme::SURFACE),
        }
    }
}

/// The mark in front of whatever holds the keyboard, in every list on screen.
const CARET: &str = "▸ ";

fn caret(focused: bool) -> String {
    if focused {
        CARET.to_string()
    } else {
        " ".repeat(CARET.width())
    }
}

fn label_line(text: &str, focused: bool, hint: &str, width: usize) -> Line<'static> {
    let style = if focused {
        Style::default()
            .fg(theme::FOCUS)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(theme::MUTED)
    };
    // Headings take the indent of the rows below them but never the caret: that
    // marks a row the keyboard is on, and a heading is not one.
    let mut spans = vec![
        Span::raw(" ".repeat(CARET.width())),
        Span::styled(text.to_string(), style),
    ];
    if focused && !hint.is_empty() {
        let room = width.saturating_sub(CARET.width() + text.width() + hint.width());
        spans.push(Span::raw(" ".repeat(room)));
        spans.push(Span::styled(hint.to_string(), theme::faint()));
    }
    Line::from(spans)
}

fn hint_line(text: &str) -> Line<'static> {
    Line::from(Span::styled(
        format!("{}{text}", " ".repeat(CARET.width())),
        theme::faint(),
    ))
}

// ------------------------------------------------------------ results column

fn draw_results(frame: &mut Frame, app: &mut App, area: Rect) {
    let [summary_area, list_area, detail_area, status_area] = Layout::vertical([
        Constraint::Length(2),
        Constraint::Min(3),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .areas(area);

    draw_summary(frame, app, summary_area);

    let mut block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(theme::border(app.focus == Focus::Results))
        .style(Style::default().bg(theme::SURFACE))
        .title_top(tab_line(app))
        .padding(Padding::horizontal(1));
    // The keys that tick, printed on the box they act on. The footer says the
    // same thing, but this is where the eye already is.
    if app.tab == Tab::Matched && app.preview.as_ref().is_some_and(|p| !p.prepared.is_empty()) {
        block = block.title_bottom(Span::styled(
            " space  tick · ^a  all · ^r  none ",
            theme::faint(),
        ));
    }
    let inner = block.inner(list_area);
    frame.render_widget(block, list_area);

    match app.tab {
        Tab::Matched => draw_matched(frame, app, inner),
        Tab::Skipped => draw_skipped(frame, app, inner),
    }
    draw_detail(frame, app, detail_area);
    draw_status(frame, app, status_area);
}

fn tab_line(app: &App) -> Line<'static> {
    let (matched, skipped) = app.preview.as_ref().map_or((0, 0), |preview| {
        (preview.prepared.len(), preview.plan.skipped.len())
    });
    // The open tab is a filled chip rather than just bold text, so which of the
    // two lists is on screen reads at a glance.
    let style = |active: bool| {
        if active {
            Style::default()
                .fg(theme::FOREGROUND)
                .bg(theme::SELECTION_BACKGROUND)
                .add_modifier(Modifier::BOLD)
        } else {
            theme::faint()
        }
    };
    let mut spans = vec![
        Span::styled(
            format!(" To rename ({matched}) "),
            style(app.tab == Tab::Matched),
        ),
        Span::raw(" "),
        Span::styled(
            format!(" Skipped ({skipped}) "),
            style(app.tab == Tab::Skipped),
        ),
    ];
    if app.focus == Focus::Results {
        spans.push(Span::styled(" ←→ ", theme::faint()));
    }
    Line::from(spans)
}

fn draw_summary(frame: &mut Frame, app: &App, area: Rect) {
    let Some(preview) = app.preview.as_ref() else {
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled("No preview yet", theme::faint()))),
            area,
        );
        return;
    };
    let plan = &preview.plan;
    let mut spans = vec![
        count_span(plan.video_count, "video", "videos", theme::FOREGROUND),
        Span::styled(" · ", theme::faint()),
        count_span(
            plan.subtitle_count,
            "subtitle",
            "subtitles",
            theme::FOREGROUND,
        ),
        Span::styled(" · ", theme::faint()),
        count_span(plan.operations.len(), "matched", "matched", theme::CERTAIN),
        Span::styled(" · ", theme::faint()),
        count_span(plan.skipped.len(), "skipped", "skipped", theme::WORKING),
        Span::styled(" · ", theme::faint()),
        count_span(plan.directory_count, "folder", "folders", theme::MUTED),
    ];
    if !preview.prepared.is_empty() {
        spans.push(Span::styled(" · ", theme::faint()));
        spans.push(Span::styled(
            format!(
                "{} of {} ticked",
                preview.ticked_count(),
                preview.prepared.len()
            ),
            Style::default().fg(theme::TICK),
        ));
    }
    frame.render_widget(
        Paragraph::new(Line::from(spans)).wrap(Wrap { trim: true }),
        area,
    );
}

fn count_span(count: usize, one: &str, many: &str, colour: Color) -> Span<'static> {
    let word = if count == 1 { one } else { many };
    Span::styled(format!("{count} {word}"), Style::default().fg(colour))
}

fn draw_matched(frame: &mut Frame, app: &mut App, area: Rect) {
    let Some(preview) = app.preview.as_mut() else {
        frame.render_widget(placeholder("Preview a folder, or press d for a demo"), area);
        return;
    };
    if preview.prepared.is_empty() {
        frame.render_widget(placeholder("Nothing to rename"), area);
        return;
    }

    // The caret takes a column from every row, highlighted or not.
    let width = (area.width as usize).saturating_sub(CARET.width());
    let items: Vec<ListItem> = preview
        .prepared
        .iter()
        .zip(&preview.ticked)
        .map(|(prepared, ticked)| {
            ListItem::new(operation_line(prepared, *ticked, &preview.plan, width))
        })
        .collect();
    let list = List::new(items).highlight_symbol(CARET).highlight_style(
        Style::default()
            .bg(theme::SELECTION_BACKGROUND)
            .add_modifier(Modifier::BOLD),
    );
    frame.render_stateful_widget(list, area, &mut preview.matched_state);
}

/// `[✓] source → target        badge`, with the badge pushed to the right.
fn operation_line(
    prepared: &PreparedOperation,
    ticked: bool,
    plan: &RenamePlan,
    width: usize,
) -> Line<'static> {
    let (marker, marker_style) = if ticked {
        ("[✓] ", Style::default().fg(theme::TICK))
    } else {
        ("[ ] ", theme::faint())
    };
    let badge = match_badge(&prepared.operation.reason);
    let badge_style = match prepared.operation.reason {
        // An episode id is as good as certain, so it can be read at a glance;
        // a fuzzy score is the one a person should actually look at, so it stays
        // quiet rather than shouting for attention.
        MatchReason::Episode(_) => Style::default().fg(theme::CERTAIN),
        MatchReason::Fuzzy(_) => theme::faint(),
    };

    let source = display_path(prepared.source(), &plan.root);
    let target = prepared
        .destination()
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let available = width.saturating_sub(marker.width() + badge.width() + 2);
    let body = fit(&format!("{source} → {target}"), available);
    let gap = available.saturating_sub(body.width()) + 2;

    Line::from(vec![
        Span::styled(marker, marker_style),
        Span::styled(body, Style::default().fg(theme::FOREGROUND)),
        Span::raw(" ".repeat(gap)),
        Span::styled(badge, badge_style),
    ])
}

fn draw_skipped(frame: &mut Frame, app: &mut App, area: Rect) {
    let Some(preview) = app.preview.as_mut() else {
        frame.render_widget(placeholder("Preview a folder, or press d for a demo"), area);
        return;
    };
    if preview.plan.skipped.is_empty() {
        frame.render_widget(placeholder("Nothing was skipped"), area);
        return;
    }

    let root = preview.plan.root.clone();
    let rows: Vec<Row> = preview
        .plan
        .skipped
        .iter()
        .map(|skipped| {
            Row::new(vec![
                Cell::from(display_path(&skipped.path, &root))
                    .style(Style::default().fg(theme::FOREGROUND)),
                Cell::from(skip_label(&skipped.reason)).style(theme::muted()),
            ])
        })
        .collect();
    let table = Table::new(
        rows,
        [Constraint::Percentage(60), Constraint::Percentage(40)],
    )
    .header(Row::new(vec!["Source", "Reason"]).style(theme::faint().add_modifier(Modifier::BOLD)))
    .highlight_symbol(CARET)
    .row_highlight_style(Style::default().bg(theme::SELECTION_BACKGROUND));
    frame.render_stateful_widget(table, area, &mut preview.skipped_state);
}

fn draw_detail(frame: &mut Frame, app: &App, area: Rect) {
    // List rows elide long paths, so the highlighted one is always spelled out
    // here in full.
    let text = app.preview.as_ref().and_then(|preview| match app.tab {
        Tab::Matched => {
            let index = preview.matched_state.selected()?;
            let prepared = preview.prepared.get(index)?;
            Some(format!(
                "{}  →  {}",
                display_path(prepared.source(), &preview.plan.root),
                display_path(prepared.destination(), &preview.plan.root)
            ))
        }
        Tab::Skipped => {
            let index = preview.skipped_state.selected()?;
            let skipped = preview.plan.skipped.get(index)?;
            Some(format!(
                "{}  —  {}",
                display_path(&skipped.path, &preview.plan.root),
                skip_label(&skipped.reason)
            ))
        }
    });
    let line = Line::from(Span::styled(text.unwrap_or_default(), theme::muted()));
    frame.render_widget(Paragraph::new(line), area);
}

fn draw_status(frame: &mut Frame, app: &App, area: Rect) {
    let colour = match app.status.kind {
        StatusKind::Ready => theme::MUTED,
        StatusKind::Working => theme::WORKING,
        StatusKind::Success => theme::SUCCESS,
        StatusKind::Demo => theme::DEMO,
        StatusKind::Error => theme::ERROR,
    };
    let mut spans = Vec::new();
    if app.busy() {
        spans.push(Span::styled(
            format!("{} ", SPINNER[app.ticks % SPINNER.len()]),
            Style::default().fg(colour),
        ));
    }
    spans.push(Span::styled(
        app.status.text.clone(),
        Style::default().fg(colour).add_modifier(Modifier::BOLD),
    ));
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn placeholder(text: &str) -> Paragraph<'static> {
    Paragraph::new(Line::from(Span::styled(text.to_string(), theme::faint())))
        .alignment(Alignment::Center)
}

// ------------------------------------------------------------------- modals

/// Every key, grouped by what it is for, with the vim spelling beside the arrows.
fn draw_help(frame: &mut Frame, area: Rect) {
    let shortcuts: [(&str, &str); 15] = [
        ("tab / shift+tab", "Next / previous control"),
        ("↑ ↓  or  k j", "Move in a control, and between them"),
        ("← →  or  h l", "Set the option, or switch tab"),
        ("home end / g G", "First / last row"),
        ("ctrl+d/u  ctrl+f/b", "Half a page, or a whole one"),
        ("i / esc", "Enter / leave the path field"),
        ("space", "Tick or untick the highlighted rename"),
        ("enter", "Preview from the path field, else as space"),
        ("ctrl+a / ctrl+r", "Tick everything / nothing"),
        ("p", "Preview"),
        ("a", "Apply the ticked renames"),
        ("d", "Demo mode"),
        ("o", "Browse for a directory"),
        ("?", "This list"),
        ("q", "Quit"),
    ];
    let mut lines = vec![
        Line::from(Span::styled(
            "Workflow: path → enter or p to preview → space to tick → a to apply",
            theme::muted(),
        )),
        Line::default(),
    ];
    lines.extend(shortcuts.iter().map(|(key, description)| {
        Line::from(vec![
            Span::styled(format!("{key:<20}"), theme::key()),
            Span::styled(
                (*description).to_string(),
                Style::default().fg(theme::FOREGROUND),
            ),
        ])
    }));
    lines.push(Line::default());
    lines.push(Line::from(Span::styled(
        "In the path field letters type: ctrl+a/e jump, ctrl+u/k/w delete.",
        theme::faint(),
    )));

    let popup = centered(area, 72, lines.len() as u16 + 4);
    render_dialog(
        frame,
        popup,
        " Keyboard shortcuts ",
        Text::from(lines),
        " esc  close ",
    );
}

fn draw_confirm(frame: &mut Frame, area: Rect, count: usize, examples: &[String]) {
    let mut lines = vec![
        Line::from(Span::styled(
            format!(
                "About to rename {count} subtitle {}.",
                if count == 1 { "file" } else { "files" }
            ),
            Style::default().fg(theme::FOREGROUND),
        )),
        Line::from(Span::styled(
            "Existing files are never overwritten.",
            theme::muted(),
        )),
        Line::default(),
    ];
    lines.extend(
        examples
            .iter()
            .map(|example| Line::from(Span::styled(example.clone(), theme::muted()))),
    );
    if count > examples.len() {
        lines.push(Line::from(Span::styled(
            format!("… and {} more", count - examples.len()),
            theme::faint(),
        )));
    }

    let popup = centered(area, 72, lines.len() as u16 + 4);
    render_dialog(
        frame,
        popup,
        " Confirm apply ",
        Text::from(lines),
        " enter  apply   esc  cancel ",
    );
}

fn draw_picker(frame: &mut Frame, area: Rect, picker: &mut crate::tui::picker::Picker) {
    let popup = centered(area, 72, 22);
    frame.render_widget(Clear, popup);
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme::FOCUS))
        .style(Style::default().bg(theme::PANEL))
        .title_top(Span::styled(" Choose a directory ", theme::heading()))
        .title_bottom(Span::styled(
            " ↑↓/jk  move   enter/l  open   ←/h  up   s  use folder   esc  cancel ",
            theme::faint(),
        ))
        .padding(Padding::horizontal(1));
    let inner = block.inner(popup);
    frame.render_widget(block, popup);

    let [current_area, list_area] =
        Layout::vertical([Constraint::Length(2), Constraint::Min(1)]).areas(inner);
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            picker.current.to_string_lossy().into_owned(),
            Style::default().fg(theme::FOREGROUND),
        )))
        .wrap(Wrap { trim: true }),
        current_area,
    );

    if let Some(error) = picker.error.clone() {
        frame.render_widget(placeholder(&error), list_area);
        return;
    }
    if picker.entries.is_empty() {
        frame.render_widget(placeholder("No subfolders here"), list_area);
        return;
    }
    let items: Vec<ListItem> = picker
        .entries
        .iter()
        .map(|entry| {
            let name = entry
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default();
            ListItem::new(Line::from(vec![
                Span::styled("📁 ", theme::faint()),
                Span::styled(name, Style::default().fg(theme::FOREGROUND)),
            ]))
        })
        .collect();
    let list = List::new(items).highlight_style(
        Style::default()
            .bg(theme::SELECTION_BACKGROUND)
            .add_modifier(Modifier::BOLD),
    );
    frame.render_stateful_widget(list, list_area, &mut picker.state);
}

fn render_dialog(frame: &mut Frame, area: Rect, title: &str, body: Text<'static>, footer: &str) {
    frame.render_widget(Clear, area);
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme::FOCUS))
        .style(Style::default().bg(theme::PANEL))
        .title_top(Span::styled(title.to_string(), theme::heading()))
        .title_bottom(Span::styled(footer.to_string(), theme::faint()))
        .padding(Padding::horizontal(1));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    frame.render_widget(Paragraph::new(body).wrap(Wrap { trim: false }), inner);
}

/// A popup of at most `width` × `height`, centred and never bigger than `area`.
fn centered(area: Rect, width: u16, height: u16) -> Rect {
    let width = width.min(area.width.saturating_sub(2));
    let height = height.min(area.height.saturating_sub(2));
    Rect::new(
        area.x + (area.width.saturating_sub(width)) / 2,
        area.y + (area.height.saturating_sub(height)) / 2,
        width,
        height,
    )
}

// -------------------------------------------------------------- text helpers

/// Trim `text` to `width` columns, keeping the end, which is the filename.
fn fit(text: &str, width: usize) -> String {
    if text.width() <= width {
        return text.to_string();
    }
    if width <= 1 {
        return "…".repeat(width);
    }
    let mut kept: Vec<char> = Vec::new();
    let mut used = 1; // the leading ellipsis
    for character in text.chars().rev() {
        let character_width = UnicodeWidthStr::width(character.to_string().as_str());
        if used + character_width > width {
            break;
        }
        used += character_width;
        kept.push(character);
    }
    let tail: String = kept.into_iter().rev().collect();
    format!("…{tail}")
}

/// Pad `text` with spaces so a highlighted row fills its width.
fn pad(text: &str, width: usize) -> String {
    let mut padded = text.to_string();
    padded.push_str(&" ".repeat(width.saturating_sub(text.width())));
    padded
}

/// Break `text` into lines of at most `width` columns, on word boundaries.
fn wrap(text: &str, width: usize) -> Vec<String> {
    let width = width.saturating_sub(2).max(8);
    let mut lines = Vec::new();
    let mut current = String::new();
    for word in text.split_whitespace() {
        if current.is_empty() {
            current = word.to_string();
        } else if current.width() + 1 + word.width() <= width {
            current.push(' ');
            current.push_str(word);
        } else {
            lines.push(std::mem::take(&mut current));
            current = word.to_string();
        }
    }
    if !current.is_empty() {
        lines.push(current);
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fit_keeps_the_end_of_a_long_path() {
        assert_eq!(fit("season one/episode.srt", 12), "…episode.srt");
        assert_eq!(fit("short.srt", 12), "short.srt");
    }

    #[test]
    fn wrap_breaks_on_words() {
        let lines = wrap("one two three four five", 12);
        assert!(lines.iter().all(|line| line.width() <= 10));
        assert_eq!(lines.concat().replace(' ', ""), "onetwothreefourfive");
    }

    #[test]
    fn pad_fills_to_the_requested_width() {
        assert_eq!(pad("ab", 5), "ab   ");
        assert_eq!(pad("abcdef", 3), "abcdef");
    }
}
