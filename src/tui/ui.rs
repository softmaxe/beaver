//! Drawing the interface.
//!
//! Layout is measured in character cells, not pixels. Everything the user
//! configures lives on the left, everything the tool reports lives on the right;
//! that split is what makes the three-step workflow legible without numbering the
//! steps. Below 100 columns the two panes cannot sit side by side, so they stack.

use ratatui::layout::{Alignment, Constraint, Layout, Position, Rect};
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
use crate::tui::app::{App, Focus, Hit, Modal, StatusKind, Tab};
use crate::tui::theme;

/// Narrower than this, the setup column and the results cannot share a row.
const WIDE_LAYOUT_MINIMUM: u16 = 100;
const SETUP_WIDTH: u16 = 38;
const SPINNER: [&str; 4] = ["⠋", "⠙", "⠹", "⠸"];

pub fn draw(frame: &mut Frame, app: &mut App) {
    app.hits.clear();
    let area = frame.area();
    frame.render_widget(Block::new().style(theme::base()), area);

    let [header_area, body_area, footer_area] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(3),
        Constraint::Length(1),
    ])
    .areas(area);

    draw_header(frame, header_area, app.hover, &mut app.hits);
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

    let hover = app.hover;
    match app.modal.as_mut() {
        Some(Modal::Help) => draw_help(frame, area, hover, &mut app.hits),
        Some(Modal::Confirm { count, examples }) => {
            draw_confirm(frame, area, *count, examples, hover, &mut app.hits)
        }
        Some(Modal::Picker(picker)) => draw_picker(frame, area, picker, hover, &mut app.hits),
        None => {}
    }
}

/// Whether the pointer rests on `rect`, for the hover tint.
fn hovered(hover: Option<Position>, rect: Rect) -> bool {
    hover.is_some_and(|point| rect.contains(point))
}

// ------------------------------------------------------------------ chrome

fn draw_header(
    frame: &mut Frame,
    area: Rect,
    hover: Option<Position>,
    hits: &mut Vec<(Rect, Hit)>,
) {
    // Help and quit are otherwise keys and nothing else, which leaves a pointer
    // with no way to reach either of them, so they take the end of the bar and
    // the tagline gives way rather than being written over.
    const TITLE: &str = " Subtitle Renamer ";
    const TAGLINE: &str = "· align subtitle filenames with the videos beside them";
    let chips = [
        (Hit::HelpButton, " Help (?) "),
        (Hit::QuitButton, " Quit (q) "),
    ];
    let total: u16 = chips.iter().map(|(_, text)| text.width() as u16).sum();
    let room = area.width.saturating_sub(total) as usize;

    let mut spans = vec![Span::styled(TITLE, theme::heading())];
    if TITLE.width() + TAGLINE.width() <= room {
        spans.push(Span::styled(TAGLINE, theme::faint()));
    }
    frame.render_widget(
        Paragraph::new(Line::from(spans)).style(Style::default().bg(theme::PANEL)),
        area,
    );

    if room < TITLE.width() {
        return;
    }
    let mut x = area.right() - total;
    for (hit, text) in chips {
        let rect = Rect::new(x, area.y, text.width() as u16, 1);
        frame.render_widget(
            Paragraph::new(Line::from(chip_span(
                text,
                hovered(hover, rect),
                theme::PANEL,
            ))),
            rect,
        );
        hits.push((rect, hit));
        x += rect.width;
    }
}

/// A quiet clickable label on the chrome: readable, but not a filled button.
///
/// It takes the background it is painted on, because the header bar and a panel
/// border are different colours and a chip that carries the wrong one shows as
/// a patch along the edge.
fn chip_span(text: &str, hovered: bool, background: Color) -> Span<'static> {
    let style = if hovered {
        Style::default()
            .fg(theme::FOREGROUND)
            .bg(theme::SELECTION_BACKGROUND)
    } else {
        theme::faint().bg(background)
    };
    Span::styled(text.to_string(), style)
}

/// Current status followed by the keys the focused control answers to.
///
/// Only keys that are *not* already printed on something visible earn a place
/// here. Every verb has a button with its key on the label, and ticking every
/// row or none of them sits on the results border, so repeating any of that
/// would just be a second copy of the screen along the bottom edge.
fn draw_footer(frame: &mut Frame, app: &App, area: Rect) {
    let mut hints: Vec<(&str, &str, Style)> = match app.focus {
        Focus::Directory => vec![
            ("enter", "preview", theme::key()),
            ("↓", "next field", theme::key()),
            ("esc", "leave field", theme::key()),
        ],
        Focus::Recursive | Focus::Strict => vec![
            ("space", "toggle", theme::key()),
            ("←→", "set", theme::key()),
            ("↑↓", "move", theme::key()),
        ],
        Focus::Level => vec![
            ("↑↓", "choose", theme::key()),
            ("space", "cycle", theme::key()),
        ],
        Focus::Results => vec![
            ("space", "tick", theme::key()),
            ("←→", "tabs", theme::key()),
        ],
    };
    for (hit, key, label) in [
        (Hit::PreviewButton, "p", "preview"),
        (Hit::ApplyButton, "a", "apply"),
        (Hit::DemoButton, "d", "demo"),
    ] {
        if !app.hits.iter().any(|(_, visible)| *visible == hit) {
            hints.push((key, label, theme::key()));
        }
    }

    // Below this width the header cannot fit its Help and Quit controls. That
    // size is unsupported, but keep the two escape hatches visible anyway.
    let cramped = area.width < 38;
    if cramped {
        hints = vec![("?", "help", theme::key()), ("q", "quit", theme::key())];
    }

    let status_width = 1 + app.status.text.width() + usize::from(app.busy()) * 2;
    while !cramped
        && !hints.is_empty()
        && status_width + 3 + hints_width(&hints).saturating_sub(1) > area.width as usize
    {
        hints.pop();
    }

    let mut spans = vec![Span::raw(" ")];
    if !cramped {
        let colour = match app.status.kind {
            StatusKind::Ready => theme::MUTED,
            StatusKind::Working => theme::WORKING,
            StatusKind::Success => theme::SUCCESS,
            StatusKind::Demo => theme::DEMO,
            StatusKind::Error => theme::ERROR,
        };
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
        if !hints.is_empty() {
            spans.push(Span::styled(" · ", theme::faint()));
        }
    }
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
    app.setup_area = area;
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
    rows.push(SetupRow::button(
        "o",
        "Browse for a folder",
        ButtonKind::Neutral,
        width,
    ));
    let browse_button_row = rows.len() - 1;
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

    // A blank row between the three keeps them reading as separate buttons
    // rather than one striped block.
    rows.push(SetupRow::button("p", "Preview", ButtonKind::Primary, width));
    let preview_button_row = rows.len() - 1;
    rows.push(SetupRow::blank());
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
    let apply_button_row = rows.len() - 1;
    rows.push(SetupRow::blank());
    rows.push(SetupRow::button(
        "d",
        "Demo mode",
        ButtonKind::Neutral,
        width,
    ));
    let demo_button_row = rows.len() - 1;

    // Taking the keyboard drags the column to whatever now holds it; between
    // those moves the wheel is free to put it anywhere, which is the only way a
    // pointer can reach the buttons in a short terminal.
    let height = inner.height as usize;
    if app.setup_focus != app.focus {
        app.setup_focus = app.focus;
        let focus_row = match app.focus {
            Focus::Directory => input_row,
            Focus::Recursive => recursive_row,
            Focus::Strict => strict_row,
            Focus::Level => level_rows[app.level.index()],
            Focus::Results => 0,
        };
        app.setup_scroll = focus_row.saturating_sub(height.saturating_sub(1));
    }
    let scroll = app.setup_scroll.min(rows.len().saturating_sub(height));
    app.setup_scroll = scroll;

    for (offset, row) in rows.iter_mut().enumerate().skip(scroll).take(height) {
        let y = inner.y + (offset - scroll) as u16;
        let line_area = Rect::new(
            inner.x + row.inset,
            y,
            inner.width.saturating_sub(2 * row.inset),
            1,
        );
        // Every control row answers a click; hint and heading rows do not.
        let hit = if offset == input_row {
            Some(Hit::Directory)
        } else if offset == browse_button_row {
            Some(Hit::BrowseButton)
        } else if offset == recursive_row {
            Some(Hit::Recursive)
        } else if offset == strict_row {
            Some(Hit::Strict)
        } else if let Some(level) = level_rows.iter().position(|row| *row == offset) {
            Some(Hit::Level(level))
        } else if offset == preview_button_row {
            Some(Hit::PreviewButton)
        } else if offset == apply_button_row {
            Some(Hit::ApplyButton)
        } else if offset == demo_button_row {
            Some(Hit::DemoButton)
        } else {
            None
        };
        let mut style = row.style;
        if let Some(hit) = hit {
            app.hits.push((line_area, hit));
            let under = hovered(app.hover, line_area);
            if row.inset > 0 {
                // A button carries its fill in the spans, so hovering lifts
                // every filled span one step up the same ramp; the row style
                // behind it is the panel, which must not move.
                if under {
                    for span in &mut row.line.spans {
                        span.style.bg = span.style.bg.map(theme::hovered_fill);
                    }
                }
            } else if under {
                // The flat controls take the selection background under the
                // pointer, which is what says "a click here would land".
                style = style.bg(theme::SELECTION_BACKGROUND);
            }
        }
        frame.render_widget(
            Paragraph::new(row.line.clone()).style(style),
            Rect::new(inner.x, y, inner.width, 1),
        );
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
    /// Columns the clickable area is held in from either edge of the column.
    inset: u16,
}

#[derive(Clone, Copy)]
enum ButtonKind {
    Primary,
    Confirm,
    Neutral,
    Disabled,
}

impl ButtonKind {
    /// Fill and text colour, in that order.
    fn colours(self) -> (Color, Color) {
        match self {
            Self::Primary => (theme::FOCUS, theme::PANEL),
            Self::Confirm => (theme::SUCCESS, theme::PANEL),
            Self::Neutral => (theme::SELECTION_BACKGROUND, theme::FOREGROUND),
            Self::Disabled => (theme::SELECTION_BACKGROUND, theme::FAINT),
        }
    }
}

/// A button reads `Label (k)`: what it does first, the key that does it after.
///
/// The two are separate spans so the key can sit back a shade without breaking
/// the fill, and the whole thing is padded to a fixed shape by its caller.
fn button_spans(label: &str, key: &str, kind: ButtonKind, hovered: bool) -> [Span<'static>; 2] {
    let (fill, foreground) = kind.colours();
    let fill = if hovered {
        theme::hovered_fill(fill)
    } else {
        fill
    };
    let base = Style::default().fg(foreground).bg(fill);
    [
        Span::styled(label.to_string(), base.add_modifier(Modifier::BOLD)),
        Span::styled(format!(" ({key})"), base),
    ]
}

/// Columns [`button_spans`] takes, plus the padding a free-standing button gets.
fn button_width(label: &str, key: &str) -> usize {
    label.width() + key.width() + 3 + 2 * BUTTON_PADDING
}

/// Blank columns held either side of a button's text.
const BUTTON_PADDING: usize = 2;

impl SetupRow {
    fn text(line: Line<'static>) -> Self {
        Self {
            line,
            style: Style::default().bg(theme::SURFACE),
            inset: 0,
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
            inset: 0,
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
            inset: 0,
        }
    }

    /// A full-width button, held off the panel edges so it reads as a control
    /// sitting on the column rather than a band painted across it.
    fn button(key: &str, label: &str, kind: ButtonKind, width: usize) -> Self {
        let fill = kind.colours().0;
        let inner = width.saturating_sub(2 * BUTTON_MARGIN);
        let text = label.width() + key.width() + 3;
        let left = inner.saturating_sub(text) / 2;
        let margin = || {
            Span::styled(
                " ".repeat(BUTTON_MARGIN),
                Style::default().bg(theme::SURFACE),
            )
        };
        let pad = |columns: usize| Span::styled(" ".repeat(columns), Style::default().bg(fill));

        let mut spans = vec![margin(), pad(left)];
        spans.extend(button_spans(label, key, kind, false));
        spans.push(pad(inner.saturating_sub(left + text)));
        spans.push(margin());
        Self {
            line: Line::from(spans),
            style: Style::default().bg(theme::SURFACE),
            inset: BUTTON_MARGIN as u16,
        }
    }
}

/// Columns of panel left bare either side of a full-width button.
const BUTTON_MARGIN: usize = 1;

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
    let [summary_area, list_area, detail_area] = Layout::vertical([
        Constraint::Length(2),
        Constraint::Min(3),
        Constraint::Length(1),
    ])
    .areas(area);

    draw_summary(frame, app, summary_area);

    // The tab chips sit on the top border; measure them off the same strings
    // the title draws, so a click lands exactly on what is painted.
    let (matched, skipped) = tab_counts(app);
    let (matched_chip, skipped_chip) = (chip("To rename", matched), chip("Skipped", skipped));
    let mut x = list_area.x + 1;
    let matched_strip = Rect::new(x, list_area.y, matched_chip.width() as u16, 1);
    x += matched_strip.width + 1;
    let skipped_strip = Rect::new(x, list_area.y, skipped_chip.width() as u16, 1);
    app.hits.push((matched_strip, Hit::Tab(Tab::Matched)));
    app.hits.push((skipped_strip, Hit::Tab(Tab::Skipped)));

    let mut block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(theme::border(app.focus == Focus::Results))
        .style(Style::default().bg(theme::SURFACE))
        .title_top(tab_line(
            app,
            hovered(app.hover, matched_strip),
            hovered(app.hover, skipped_strip),
        ))
        .padding(Padding::horizontal(1));
    // Ticking every row or none of them is a chord, so it gets clickable chips
    // on the box it acts on. The footer no longer repeats them.
    if app.tab == Tab::Matched
        && list_area.height >= 3
        && app.preview.as_ref().is_some_and(|p| !p.prepared.is_empty())
    {
        const ALL: &str = " Tick all (^a) ";
        const NONE: &str = " Tick none (^r) ";
        let y = list_area.bottom() - 1;
        let all_strip = Rect::new(list_area.x + 1, y, ALL.width() as u16, 1);
        let none_strip = Rect::new(all_strip.right() + 1, y, NONE.width() as u16, 1);
        block = block.title_bottom(Line::from(vec![
            chip_span(ALL, hovered(app.hover, all_strip), theme::SURFACE),
            Span::raw(" "),
            chip_span(NONE, hovered(app.hover, none_strip), theme::SURFACE),
        ]));
        app.hits.push((all_strip, Hit::TickAll));
        app.hits.push((none_strip, Hit::TickNone));
    }
    let inner = block.inner(list_area);
    frame.render_widget(block, list_area);

    match app.tab {
        Tab::Matched => draw_matched(frame, app, inner),
        Tab::Skipped => draw_skipped(frame, app, inner),
    }
    draw_detail(frame, app, detail_area);
}

fn tab_counts(app: &App) -> (usize, usize) {
    app.preview.as_ref().map_or((0, 0), |preview| {
        (preview.prepared.len(), preview.plan.skipped.len())
    })
}

/// The painted text of a tab chip, shared by the title and its click strip.
fn chip(label: &str, count: usize) -> String {
    format!(" {label} ({count}) ")
}

fn tab_line(app: &App, matched_hover: bool, skipped_hover: bool) -> Line<'static> {
    let (matched, skipped) = tab_counts(app);
    // The open tab is a filled chip rather than just bold text, so which of the
    // two lists is on screen reads at a glance. Under the pointer an inactive
    // chip takes the same fill without the bold: clickable, but not where the
    // keyboard is.
    let style = |active: bool, under_pointer: bool| {
        if active {
            Style::default()
                .fg(theme::FOREGROUND)
                .bg(theme::SELECTION_BACKGROUND)
                .add_modifier(Modifier::BOLD)
        } else if under_pointer {
            Style::default()
                .fg(theme::FOREGROUND)
                .bg(theme::SELECTION_BACKGROUND)
        } else {
            theme::faint()
        }
    };
    let mut spans = vec![
        Span::styled(
            chip("To rename", matched),
            style(app.tab == Tab::Matched, matched_hover),
        ),
        Span::raw(" "),
        Span::styled(
            chip("Skipped", skipped),
            style(app.tab == Tab::Skipped, skipped_hover),
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
    // Hover tinting needs each visible row's rectangle before the list is
    // rendered, so it works off the offset of the previous frame; a frame that
    // scrolls redraws immediately anyway.
    let count = preview.prepared.len();
    let assumed_offset = preview.matched_state.offset().min(count.saturating_sub(1));
    let rows_on_screen = area.height as usize;
    let items: Vec<ListItem> = preview
        .prepared
        .iter()
        .zip(&preview.ticked)
        .enumerate()
        .map(|(index, (prepared, ticked))| {
            let mut item = ListItem::new(operation_line(prepared, *ticked, &preview.plan, width));
            if index >= assumed_offset
                && index < assumed_offset + rows_on_screen
                && hovered(
                    app.hover,
                    Rect::new(
                        area.x,
                        area.y + (index - assumed_offset) as u16,
                        area.width,
                        1,
                    ),
                )
            {
                item = item.style(Style::default().bg(theme::SELECTION_BACKGROUND));
            }
            item
        })
        .collect();
    let list = List::new(items).highlight_symbol(CARET).highlight_style(
        Style::default()
            .bg(theme::SELECTION_BACKGROUND)
            .add_modifier(Modifier::BOLD),
    );
    frame.render_stateful_widget(list, area, &mut preview.matched_state);

    // Register from the offset as rendered, so clicks land on what is shown.
    for index in
        preview.matched_state.offset()..count.min(preview.matched_state.offset() + rows_on_screen)
    {
        let row = Rect::new(
            area.x,
            area.y + (index - preview.matched_state.offset()) as u16,
            area.width,
            1,
        );
        app.hits.push((row, Hit::MatchedRow(index)));
    }
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
    // The header takes the first line, so data rows start one below; hover and
    // click rectangles follow it.
    let assumed_offset = preview.skipped_state.offset();
    let count = preview.plan.skipped.len();
    let rows_on_screen = area.height.saturating_sub(1) as usize;
    let rows: Vec<Row> = preview
        .plan
        .skipped
        .iter()
        .enumerate()
        .map(|(index, skipped)| {
            let mut row = Row::new(vec![
                Cell::from(display_path(&skipped.path, &root))
                    .style(Style::default().fg(theme::FOREGROUND)),
                Cell::from(skip_label(&skipped.reason)).style(theme::muted()),
            ]);
            if index >= assumed_offset
                && index < assumed_offset + rows_on_screen
                && hovered(
                    app.hover,
                    Rect::new(
                        area.x,
                        area.y + 1 + (index - assumed_offset) as u16,
                        area.width,
                        1,
                    ),
                )
            {
                row = row.style(Style::default().bg(theme::SELECTION_BACKGROUND));
            }
            row
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

    for index in
        preview.skipped_state.offset()..count.min(preview.skipped_state.offset() + rows_on_screen)
    {
        let row = Rect::new(
            area.x,
            area.y + 1 + (index - preview.skipped_state.offset()) as u16,
            area.width,
            1,
        );
        app.hits.push((row, Hit::SkippedRow(index)));
    }
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

fn placeholder(text: &str) -> Paragraph<'static> {
    Paragraph::new(Line::from(Span::styled(text.to_string(), theme::faint())))
        .alignment(Alignment::Center)
}

// ------------------------------------------------------------------- modals

/// Every dialog is the same width, so they never jump around between steps.
const DIALOG_WIDTH: u16 = 72;

/// Every key, grouped by what it is for, with the vim spelling beside the arrows.
fn draw_help(frame: &mut Frame, area: Rect, hover: Option<Position>, hits: &mut Vec<(Rect, Hit)>) {
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

    let popup = centered(area, DIALOG_WIDTH, lines.len() as u16 + 4);
    render_dialog(
        frame,
        popup,
        " Keyboard shortcuts ",
        Text::from(lines),
        &[(Hit::HelpClose, "Close", "esc", ButtonKind::Neutral)],
        hover,
        hits,
    );
}

fn draw_confirm(
    frame: &mut Frame,
    area: Rect,
    count: usize,
    examples: &[String],
    hover: Option<Position>,
    hits: &mut Vec<(Rect, Hit)>,
) {
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
    // Wrapped here rather than by the paragraph, so the dialog is sized for the
    // rows it actually needs and a long rename is never cut off at the bottom.
    lines.extend(
        examples
            .iter()
            .flat_map(|example| wrap(example, DIALOG_WIDTH as usize - 2))
            .map(|line| Line::from(Span::styled(line, theme::muted()))),
    );
    if count > examples.len() {
        lines.push(Line::from(Span::styled(
            format!("… and {} more", count - examples.len()),
            theme::faint(),
        )));
    }

    let popup = centered(area, DIALOG_WIDTH, lines.len() as u16 + 4);
    render_dialog(
        frame,
        popup,
        " Confirm apply ",
        Text::from(lines),
        &[
            (Hit::ConfirmCancel, "Cancel", "esc", ButtonKind::Neutral),
            (Hit::ConfirmApply, "Apply", "enter", ButtonKind::Confirm),
        ],
        hover,
        hits,
    );
}

fn draw_picker(
    frame: &mut Frame,
    area: Rect,
    picker: &mut crate::tui::picker::Picker,
    hover: Option<Position>,
    hits: &mut Vec<(Rect, Hit)>,
) {
    let popup = centered(area, DIALOG_WIDTH, 22);
    frame.render_widget(Clear, popup);
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme::FOCUS))
        .style(Style::default().bg(theme::PANEL))
        .title_top(Span::styled(" Choose a directory ", theme::heading()))
        .title_bottom(Span::styled(
            " ↑↓/jk  move   enter/l  open ",
            theme::faint(),
        ))
        .padding(Padding::horizontal(1));
    let inner = block.inner(popup);
    frame.render_widget(block, popup);

    let [current_area, list_area, _, buttons_area] = Layout::vertical([
        Constraint::Length(2),
        Constraint::Min(1),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .areas(inner);
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            picker.current.to_string_lossy().into_owned(),
            Style::default().fg(theme::FOREGROUND),
        )))
        .wrap(Wrap { trim: true }),
        current_area,
    );

    // Going up a level, taking this folder and backing out are all keys
    // otherwise, so each one gets a button; the buttons are drawn before the
    // listing can bail out, because an unreadable folder is exactly when a way
    // back up matters most.
    draw_dialog_buttons(
        frame,
        buttons_area,
        &[
            (
                Hit::PickerParent,
                "Parent folder",
                "h",
                if picker.current.parent().is_some() {
                    ButtonKind::Neutral
                } else {
                    ButtonKind::Disabled
                },
            ),
            (Hit::PickerCancel, "Cancel", "esc", ButtonKind::Neutral),
            (Hit::PickerUse, "Use this folder", "s", ButtonKind::Primary),
        ],
        hover,
        hits,
    );

    if let Some(error) = picker.error.clone() {
        frame.render_widget(placeholder(&error), list_area);
        return;
    }
    if picker.entries.is_empty() {
        frame.render_widget(placeholder("No subfolders here"), list_area);
        return;
    }
    let count = picker.entries.len();
    let assumed_offset = picker.state.offset();
    let rows_on_screen = list_area.height as usize;
    let items: Vec<ListItem> = picker
        .entries
        .iter()
        .enumerate()
        .map(|(index, entry)| {
            let name = entry
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default();
            let mut item = ListItem::new(Line::from(vec![
                Span::styled("📁 ", theme::faint()),
                Span::styled(name, Style::default().fg(theme::FOREGROUND)),
            ]));
            if index >= assumed_offset
                && index < assumed_offset + rows_on_screen
                && hovered(
                    hover,
                    Rect::new(
                        list_area.x,
                        list_area.y + (index - assumed_offset) as u16,
                        list_area.width,
                        1,
                    ),
                )
            {
                item = item.style(Style::default().bg(theme::SELECTION_BACKGROUND));
            }
            item
        })
        .collect();
    let list = List::new(items).highlight_style(
        Style::default()
            .bg(theme::SELECTION_BACKGROUND)
            .add_modifier(Modifier::BOLD),
    );
    frame.render_stateful_widget(list, list_area, &mut picker.state);

    for index in picker.state.offset()..count.min(picker.state.offset() + rows_on_screen) {
        let row = Rect::new(
            list_area.x,
            list_area.y + (index - picker.state.offset()) as u16,
            list_area.width,
            1,
        );
        hits.push((row, Hit::PickerRow(index)));
    }
}

fn render_dialog(
    frame: &mut Frame,
    area: Rect,
    title: &str,
    body: Text<'static>,
    buttons: &[DialogButton],
    hover: Option<Position>,
    hits: &mut Vec<(Rect, Hit)>,
) {
    frame.render_widget(Clear, area);
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme::FOCUS))
        .style(Style::default().bg(theme::PANEL))
        .title_top(Span::styled(title.to_string(), theme::heading()))
        .padding(Padding::horizontal(1));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    // The buttons sit inside the dialog with a blank row above them, so they
    // read as things to press rather than as a caption on the border.
    let [body_area, _, buttons_area] = Layout::vertical([
        Constraint::Min(1),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .areas(inner);
    frame.render_widget(Paragraph::new(body).wrap(Wrap { trim: false }), body_area);
    draw_dialog_buttons(frame, buttons_area, buttons, hover, hits);
}

/// What it does, the key that does it, and how loudly it is painted.
type DialogButton = (Hit, &'static str, &'static str, ButtonKind);

/// A right-aligned row of buttons along the bottom of a dialog.
fn draw_dialog_buttons(
    frame: &mut Frame,
    area: Rect,
    buttons: &[DialogButton],
    hover: Option<Position>,
    hits: &mut Vec<(Rect, Hit)>,
) {
    const GAP: u16 = 2;
    if area.height == 0 {
        return;
    }
    // Too narrow for the whole row, so the leftmost buttons give way one at a
    // time. Dropping the lot would strand a mouse-only user inside the dialog
    // with no way out, so the last one — the primary action — always stays and
    // the keys of the dropped ones still work.
    let mut buttons = buttons;
    let width_of = |row: &[DialogButton]| -> u16 {
        row.iter()
            .map(|(_, label, key, _)| button_width(label, key) as u16)
            .sum::<u16>()
            + GAP * row.len().saturating_sub(1) as u16
    };
    while buttons.len() > 1 && width_of(buttons) > area.width {
        buttons = &buttons[1..];
    }
    let total = width_of(buttons);
    if total > area.width {
        return;
    }
    let widths: Vec<u16> = buttons
        .iter()
        .map(|(_, label, key, _)| button_width(label, key) as u16)
        .collect();
    let mut x = area.right() - total;
    for ((hit, label, key, kind), width) in buttons.iter().zip(widths) {
        let rect = Rect::new(x, area.y, width, 1);
        let under = hovered(hover, rect);
        let fill = if under {
            theme::hovered_fill(kind.colours().0)
        } else {
            kind.colours().0
        };
        let pad = Span::styled(" ".repeat(BUTTON_PADDING), Style::default().bg(fill));
        let mut spans = vec![pad.clone()];
        spans.extend(button_spans(label, key, *kind, under));
        spans.push(pad);
        frame.render_widget(Paragraph::new(Line::from(spans)), rect);
        hits.push((rect, *hit));
        x += width + GAP;
    }
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
