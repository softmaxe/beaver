//! Drawing the interface.
//!
//! One step is on screen at a time. A bar of dots across the top says where the
//! wizard is, a single card underneath holds everything that step needs, and the
//! footer says what just happened and which keys the focused control answers to.
//! Nothing else competes for attention: there is exactly one card, and the
//! keyboard is always somewhere inside it.
//!
//! Layout is measured in character cells, not pixels. The smallest supported
//! terminal is 80 × 24.

use ratatui::layout::{Constraint, Layout, Margin, Position, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, BorderType, Clear, List, ListItem, Padding, Paragraph, Wrap};
use ratatui::Frame;
use unicode_width::UnicodeWidthStr;

use crate::applying::PreparedOperation;
use crate::paths::display_path;
use crate::planning::{MatchReason, RenamePlan};
use crate::presentation::{match_badge, plural, skip_label, MatchLevel};
use crate::tui::app::{App, Control, Hit, Modal, Outcome, StatusKind, Step};
use crate::tui::theme;

/// Columns one step of the bar takes, dot and label together.
const DOT_STRIDE: usize = 12;
/// The widest a card ever gets, so a line of text never runs the full terminal.
const CARD_WIDTH: u16 = 62;
/// The preview holds a list of filenames, so it is allowed to be wider.
const PREVIEW_WIDTH: u16 = 100;
/// Shorter than this, the blank rows around the card go to the card.
const AIR_MINIMUM: u16 = 26;
const SPINNER: [&str; 4] = ["⠋", "⠙", "⠹", "⠸"];
/// The mark in front of whatever holds the keyboard.
const CARET: &str = "▸ ";

pub fn draw(frame: &mut Frame, app: &mut App) {
    app.hits.clear();
    let area = frame.area();
    frame.render_widget(Block::new().style(theme::base()), area);

    let air = u16::from(area.height >= AIR_MINIMUM);
    let [header_area, _, steps_area, _, body_area, footer_area] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(air),
        Constraint::Length(2),
        Constraint::Length(air),
        Constraint::Min(5),
        Constraint::Length(1),
    ])
    .areas(area);

    draw_header(frame, app, header_area);
    draw_steps(frame, app, steps_area);
    draw_card(frame, app, body_area.inner(Margin::new(1, 0)));
    draw_footer(frame, app, footer_area);

    let hover = app.hover;
    match app.modal.as_mut() {
        Some(Modal::Help) => draw_help(frame, area, hover, &mut app.hits),
        Some(Modal::Confirm { count, examples }) => {
            draw_confirm(frame, area, *count, examples, hover, &mut app.hits)
        }
        Some(Modal::Picker(picker)) => draw_picker(frame, area, picker, hover, &mut app.hits),
        Some(Modal::Skipped(_)) => draw_skipped(frame, area, app),
        None => {}
    }
}

/// Whether the pointer rests on `rect`, for the hover tint.
fn hovered(hover: Option<Position>, rect: Rect) -> bool {
    hover.is_some_and(|point| rect.contains(point))
}

// ------------------------------------------------------------------ chrome

fn draw_header(frame: &mut Frame, app: &mut App, area: Rect) {
    let bar = Style::default().bg(theme::PANEL);
    frame.render_widget(Block::new().style(bar), area);

    let title = Line::from(vec![
        Span::raw(" "),
        Span::styled(
            "beaver",
            Style::default()
                .fg(theme::HEADING)
                .add_modifier(Modifier::BOLD)
                .bg(theme::PANEL),
        ),
        Span::styled(
            "  rename subtitles to match their videos",
            theme::faint().bg(theme::PANEL),
        ),
    ]);
    frame.render_widget(Paragraph::new(title), area);

    // Two quiet chips on the right, so help and quit are never only a key.
    let chips: [(Hit, &str, &str); 2] = [(Hit::Help, "help", "?"), (Hit::Quit, "quit", "q")];
    let widths: Vec<u16> = chips
        .iter()
        .map(|(_, label, key)| (label.width() + key.width() + 4) as u16)
        .collect();
    let total: u16 = widths.iter().sum::<u16>() + 1;
    if total >= area.width {
        return;
    }
    let mut x = area.right() - total;
    for ((hit, label, key), width) in chips.iter().zip(widths) {
        let rect = Rect::new(x, area.y, width, 1);
        let under = hovered(app.hover, rect);
        let background = if under {
            theme::SELECTION_BACKGROUND
        } else {
            theme::PANEL
        };
        let line = Line::from(vec![
            Span::styled(
                format!(" {key} "),
                Style::default()
                    .fg(theme::KEY)
                    .bg(background)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("{label} "),
                Style::default().fg(theme::MUTED).bg(background),
            ),
        ]);
        frame.render_widget(Paragraph::new(line), rect);
        app.hits.push((rect, *hit));
        x += width;
    }
}

/// The bar of dots: where the wizard is, what it has been through, what is left.
///
/// Every label is centred on its own dot, so a step reads as one column rather
/// than a dot with text hanging off its left edge.
fn draw_steps(frame: &mut Frame, app: &mut App, area: Rect) {
    if area.height < 2 {
        return;
    }
    let current = app.step.index();
    // The first label would run off the left of its own dot, so the whole row of
    // dots starts that far in and the labels keep their centres.
    let lead = Step::ORDER[0].label().width().saturating_sub(1) / 2;

    let mut dots: Vec<Span> = Vec::new();
    let mut labels: Vec<Span> = Vec::new();
    // Where each step's label starts and ends on the second line, so the hit
    // cells can cover the dot and its label together.
    let mut label_spans: Vec<(usize, usize)> = Vec::new();
    let mut used = 0usize;

    for (index, step) in Step::ORDER.into_iter().enumerate() {
        let (dot, dot_style, label_style) = match index.cmp(&current) {
            std::cmp::Ordering::Less => (
                "●",
                Style::default().fg(theme::SUCCESS),
                Style::default().fg(theme::MUTED),
            ),
            std::cmp::Ordering::Equal => (
                "●",
                Style::default().fg(theme::FOCUS),
                Style::default()
                    .fg(theme::FOCUS)
                    .add_modifier(Modifier::BOLD),
            ),
            std::cmp::Ordering::Greater => ("○", theme::faint(), theme::faint()),
        };
        dots.push(Span::styled(dot, dot_style));
        if index + 1 < Step::ORDER.len() {
            let connector = if index < current {
                Style::default().fg(theme::SUCCESS)
            } else {
                theme::faint()
            };
            dots.push(Span::styled("─".repeat(DOT_STRIDE - 1), connector));
        }

        let label = step.label();
        let width = label.width();
        // Centre on the dot, but never on top of the label before it.
        let dot_column = index * DOT_STRIDE + lead;
        let begin = dot_column
            .saturating_sub(width.saturating_sub(1) / 2)
            .max(used);
        labels.push(Span::raw(" ".repeat(begin - used)));
        labels.push(Span::styled(label.to_string(), label_style));
        used = begin + width;
        label_spans.push((begin, used));
    }

    let dots_width = lead + (Step::ORDER.len() - 1) * DOT_STRIDE + 1;
    let content = dots_width.max(used) as u16;
    let x = area.x + area.width.saturating_sub(content) / 2;
    let width = content.min(area.width);

    frame.render_widget(
        Paragraph::new(Line::from(dots)),
        Rect::new(
            (x + lead as u16).min(area.right().saturating_sub(1)),
            area.y,
            width.saturating_sub(lead as u16),
            1,
        ),
    );
    frame.render_widget(
        Paragraph::new(Line::from(labels)),
        Rect::new(x, area.y + 1, width, 1),
    );

    // A dot walks back to a step already done. Forward has preconditions, so the
    // card's own button owns that direction.
    for (index, (begin, end)) in label_spans.into_iter().enumerate() {
        let dot_column = index * DOT_STRIDE + lead;
        let left = x + begin.min(dot_column) as u16;
        let right = x + end.max(dot_column + 1) as u16;
        if left >= area.right() {
            break;
        }
        let cell = Rect::new(left, area.y, right.min(area.right()) - left, 2);
        app.hits.push((cell, Hit::Dot(index)));
    }
}

fn draw_footer(frame: &mut Frame, app: &App, area: Rect) {
    let bar = Style::default().bg(theme::PANEL);
    frame.render_widget(Block::new().style(bar), area);

    let colour = match app.status.kind {
        StatusKind::Ready => theme::MUTED,
        StatusKind::Working => theme::WORKING,
        StatusKind::Success => theme::SUCCESS,
        StatusKind::Error => theme::ERROR,
    };
    let mut spans = vec![Span::raw(" ")];
    if app.busy() {
        spans.push(Span::styled(
            format!("{} ", SPINNER[app.ticks % SPINNER.len()]),
            Style::default().fg(theme::WORKING).bg(theme::PANEL),
        ));
    }
    spans.push(Span::styled(
        app.status.text.clone(),
        Style::default().fg(colour).bg(theme::PANEL),
    ));
    frame.render_widget(Paragraph::new(Line::from(spans)), area);

    let hints = hints_for(app);
    let mut right: Vec<Span> = Vec::new();
    for (index, (key, label)) in hints.iter().enumerate() {
        if index > 0 {
            right.push(Span::styled(
                " ·",
                Style::default().fg(theme::BORDER).bg(theme::PANEL),
            ));
        }
        right.push(Span::styled(
            format!(" {key} "),
            Style::default()
                .fg(theme::KEY)
                .bg(theme::PANEL)
                .add_modifier(Modifier::BOLD),
        ));
        right.push(Span::styled(
            (*label).to_string(),
            Style::default().fg(theme::FAINT).bg(theme::PANEL),
        ));
    }
    let width: usize = right.iter().map(|span| span.content.width()).sum();
    if width + 2 >= area.width as usize {
        return;
    }
    let rect = Rect::new(area.right() - width as u16 - 1, area.y, width as u16 + 1, 1);
    frame.render_widget(Paragraph::new(Line::from(right)), rect);
}

/// The keys the focused control answers to right now, and nothing else.
fn hints_for(app: &App) -> Vec<(&'static str, &'static str)> {
    if app.busy() {
        return vec![("esc", "back")];
    }
    match (app.step, app.control()) {
        (_, Some(Control::Path)) => vec![("↵", "next"), ("o", "browse"), ("↓", "leave")],
        (Step::Folder, _) => vec![("↹", "move"), ("␣", "press"), ("↵", "next")],
        (Step::Rules, Some(Control::Level)) => {
            vec![("↑↓", "level"), ("←", "back"), ("↵", "preview")]
        }
        (Step::Rules, _) => vec![("␣", "toggle"), ("←", "back"), ("↵", "preview")],
        (Step::Preview, Some(Control::List)) => vec![
            ("␣", "tick"),
            ("^a/^r", "all/none"),
            ("s", "skipped"),
            ("←", "back"),
            ("a", "apply"),
        ],
        (Step::Preview, _) => vec![("↹", "move"), ("←", "back"), ("a", "apply")],
        (Step::Apply, _) => vec![("↵", "start over"), ("q", "quit")],
    }
}

// -------------------------------------------------------------------- the card

/// One rendered row of a card, and what a click on it means.
struct Row {
    line: Line<'static>,
    /// A row painted as a control carries its own fill in the spans.
    filled: bool,
    hit: Option<Hit>,
}

impl Row {
    fn text(line: Line<'static>) -> Self {
        Self {
            line,
            filled: false,
            hit: None,
        }
    }

    fn blank() -> Self {
        Self::text(Line::default())
    }

    fn on(mut self, hit: Hit) -> Self {
        self.hit = Some(hit);
        self
    }

    /// Mark the row as carrying its own fill, so a hover lifts the spans
    /// rather than painting a block behind them.
    fn filled(mut self) -> Self {
        self.filled = true;
        self
    }
}

fn draw_card(frame: &mut Frame, app: &mut App, area: Rect) {
    let wide = app.step == Step::Preview;
    let want = if wide { PREVIEW_WIDTH } else { CARD_WIDTH };
    let width = want.min(area.width);
    let height = if wide {
        area.height
    } else {
        card_height(app).min(area.height)
    };
    let rect = Rect::new(
        area.x + area.width.saturating_sub(width) / 2,
        area.y + area.height.saturating_sub(height) / 2,
        width,
        height,
    );

    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme::FOCUS))
        .style(Style::default().bg(theme::SURFACE))
        .title_top(Span::styled(
            format!(" {} ", app.step.title()),
            theme::heading(),
        ))
        .padding(Padding::new(2, 2, 1, 1));
    let inner = block.inner(rect);
    frame.render_widget(block, rect);
    if inner.width == 0 || inner.height == 0 {
        return;
    }

    match app.step {
        Step::Folder => draw_folder(frame, app, inner),
        Step::Rules => draw_rules(frame, app, inner),
        Step::Preview => draw_preview(frame, app, inner),
        Step::Apply => draw_apply(frame, app, inner),
    }
}

/// Rows the fixed-size cards take, borders and padding included.
fn card_height(app: &App) -> u16 {
    match app.step {
        Step::Folder => 10,
        Step::Rules => 13,
        Step::Apply => 11,
        Step::Preview => 20,
    }
}

/// Render `rows` from the top of `area`, registering every clickable one.
fn render_rows(frame: &mut Frame, app: &mut App, area: Rect, mut rows: Vec<Row>) {
    for (offset, row) in rows.iter_mut().enumerate().take(area.height as usize) {
        let rect = Rect::new(area.x, area.y + offset as u16, area.width, 1);
        if let Some(hit) = row.hit {
            app.hits.push((rect, hit));
            let under = hovered(app.hover, rect);
            if under {
                if row.filled {
                    for span in &mut row.line.spans {
                        span.style.bg = span.style.bg.map(theme::hovered_fill);
                    }
                } else {
                    frame.render_widget(
                        Block::new().style(Style::default().bg(theme::SELECTION_BACKGROUND)),
                        rect,
                    );
                }
            }
        }
        frame.render_widget(Paragraph::new(row.line.clone()), rect);
    }
}

/// A row that spans the card and is filled when it holds the keyboard.
fn control_row(text: Line<'static>, focused: bool, width: usize) -> Row {
    let background = if focused {
        theme::SELECTION_BACKGROUND
    } else {
        theme::SURFACE
    };
    let mut spans = vec![Span::styled(
        caret(focused),
        Style::default().fg(theme::FOCUS).bg(background),
    )];
    let mut used = CARET.width();
    for span in text.spans {
        used += span.content.width();
        spans.push(Span::styled(span.content, span.style.bg(background)));
    }
    spans.push(Span::styled(
        " ".repeat(width.saturating_sub(used)),
        Style::default().bg(background),
    ));
    Row::text(Line::from(spans)).filled()
}

fn caret(focused: bool) -> String {
    if focused {
        CARET.to_string()
    } else {
        " ".repeat(CARET.width())
    }
}

fn heading_row(text: &str) -> Row {
    Row::text(Line::from(vec![
        Span::raw(" ".repeat(CARET.width())),
        Span::styled(text.to_string(), theme::faint()),
    ]))
}

// --------------------------------------------------------------- step 1: folder

fn draw_folder(frame: &mut Frame, app: &mut App, area: Rect) {
    let width = area.width as usize;
    let focused = app.is_focused(Control::Path);
    let field_width = width.saturating_sub(CARET.width());
    let (visible, cursor) = app.directory.view(field_width);
    let empty = visible.is_empty();
    let text = if empty {
        "~/Videos/Some.Show".to_string()
    } else {
        visible
    };
    let style = if empty {
        theme::faint()
    } else {
        Style::default().fg(theme::FOREGROUND)
    };

    let rows = vec![
        heading_row("Folder to scan"),
        control_row(
            Line::from(Span::styled(pad(&text, field_width), style)),
            focused,
            width,
        )
        .on(Hit::Control(Control::Path)),
        Row::blank(),
        control_row(
            button_line("Browse", "o"),
            app.is_focused(Control::Browse),
            width,
        )
        .on(Hit::Control(Control::Browse)),
    ];
    render_rows(frame, app, area, rows);

    // The real terminal cursor, so the path field blinks like every other prompt.
    if focused && area.height > 1 {
        let x = area.x + CARET.width() as u16 + cursor as u16;
        frame.set_cursor_position(Position::new(x.min(area.right() - 1), area.y + 1));
    }

    draw_card_buttons(
        frame,
        app,
        Rect::new(area.x, area.bottom() - 1, area.width, 1),
    );
}

// ---------------------------------------------------------------- step 2: rules

fn draw_rules(frame: &mut Frame, app: &mut App, area: Rect) {
    let width = area.width as usize;
    let on_levels = app.is_focused(Control::Level);
    let mut rows = vec![heading_row("Match level")];
    for (index, level) in MatchLevel::ALL.into_iter().enumerate() {
        let chosen = level == app.level;
        let marker = if chosen { "(●) " } else { "( ) " };
        let label_style = if chosen {
            Style::default()
                .fg(theme::FOREGROUND)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme::MUTED)
        };
        let line = Line::from(vec![
            Span::styled(
                marker,
                Style::default().fg(if chosen { theme::FOCUS } else { theme::FAINT }),
            ),
            Span::styled(pad(level.label(), 11), label_style),
            Span::styled(level.hint().to_string(), theme::faint()),
        ]);
        rows.push(control_row(line, on_levels && chosen, width).on(Hit::LevelRow(index)));
    }
    rows.push(Row::blank());
    rows.push(heading_row("Scope"));
    let ticked = app.recursive;
    rows.push(
        control_row(
            Line::from(vec![
                Span::styled(
                    if ticked { "[✓] " } else { "[ ] " },
                    Style::default().fg(if ticked { theme::TICK } else { theme::FAINT }),
                ),
                Span::styled("Include subfolders", Style::default().fg(theme::FOREGROUND)),
            ]),
            app.is_focused(Control::Recursive),
            width,
        )
        .on(Hit::Control(Control::Recursive)),
    );
    render_rows(frame, app, area, rows);
    draw_card_buttons(
        frame,
        app,
        Rect::new(area.x, area.bottom() - 1, area.width, 1),
    );
}

// -------------------------------------------------------------- step 3: preview

fn draw_preview(frame: &mut Frame, app: &mut App, area: Rect) {
    if app.scanning {
        let message = format!(
            "{} Scanning {}",
            SPINNER[app.ticks % SPINNER.len()],
            app.directory.value()
        );
        draw_centred(
            frame,
            area,
            &fit(&message, area.width as usize),
            theme::WORKING,
        );
        return;
    }
    let Some(preview) = app.preview.as_ref() else {
        draw_centred(
            frame,
            area,
            "No preview yet — press ← and preview again",
            theme::FAINT,
        );
        return;
    };

    let matched = preview.prepared.len();
    let ticked = preview.ticked_count();
    let skipped = preview.plan.skipped.len();

    // Summary on the left, the two bulk keys on the right: four numbers do not
    // need four boxes, and the keys belong beside the list they act on.
    let summary = vec![
        Span::styled(
            format!("{ticked}"),
            Style::default()
                .fg(theme::TICK)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(format!(" of {matched} ticked"), theme::muted()),
    ];
    let summary_line = Line::from(summary);
    frame.render_widget(
        Paragraph::new(summary_line),
        Rect::new(area.x, area.y, area.width, 1),
    );

    let mut right: Vec<(Hit, String)> = vec![
        (Hit::TickAll, "all ^a".into()),
        (Hit::TickNone, "none ^r".into()),
    ];
    if skipped > 0 {
        right.insert(0, (Hit::Skipped, format!("{skipped} skipped  s")));
    }
    let mut x = area.right();
    for (hit, label) in right.iter().rev() {
        let width = label.width() as u16 + 2;
        if x < area.x + width {
            break;
        }
        x -= width;
        let rect = Rect::new(x, area.y, width, 1);
        let under = hovered(app.hover, rect);
        let background = if under {
            theme::SELECTION_BACKGROUND
        } else {
            theme::SURFACE
        };
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                format!(" {label} "),
                Style::default().fg(theme::FAINT).bg(background),
            ))),
            rect,
        );
        app.hits.push((rect, *hit));
    }

    // The list, the full path of the highlighted row, then the buttons.
    let [_, _, list_area, _, detail_area, buttons_area] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Min(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .areas(area);

    if matched == 0 {
        draw_centred(
            frame,
            list_area,
            "Nothing matched — press ← and loosen the match level",
            theme::FAINT,
        );
    } else {
        let width = list_area.width as usize;
        let items: Vec<ListItem> = preview
            .prepared
            .iter()
            .enumerate()
            .map(|(index, prepared)| {
                ListItem::new(operation_line(
                    prepared,
                    preview.ticked[index],
                    &preview.plan,
                    width,
                ))
            })
            .collect();
        let selected = preview.state.selected();
        let list = List::new(items).highlight_style(
            Style::default()
                .bg(theme::SELECTION_BACKGROUND)
                .add_modifier(Modifier::BOLD),
        );
        let mut state = preview.state;
        frame.render_stateful_widget(list, list_area, &mut state);
        let offset = state.offset();
        app.preview.as_mut().unwrap().state = state;

        for visible in 0..list_area.height as usize {
            let index = offset + visible;
            if index >= matched {
                break;
            }
            app.hits.push((
                Rect::new(
                    list_area.x,
                    list_area.y + visible as u16,
                    list_area.width,
                    1,
                ),
                Hit::Row(index),
            ));
        }

        // The detail line spells the highlighted row out in full: the list trims
        // long paths from the left, and this is where the whole one is readable.
        if let Some(index) = selected {
            let preview = app.preview.as_ref().unwrap();
            if let Some(prepared) = preview.prepared.get(index) {
                let text = format!(
                    "{}  →  {}",
                    display_path(prepared.source(), &preview.plan.root),
                    crate::paths::file_name(prepared.destination())
                );
                frame.render_widget(
                    Paragraph::new(Line::from(Span::styled(
                        fit(&text, detail_area.width as usize),
                        theme::faint(),
                    ))),
                    detail_area,
                );
            }
        }
    }

    draw_card_buttons(frame, app, buttons_area);
}

fn operation_line(
    prepared: &PreparedOperation,
    ticked: bool,
    plan: &RenamePlan,
    width: usize,
) -> Line<'static> {
    let badge = match_badge(&prepared.operation.reason);
    let certain = matches!(prepared.operation.reason, MatchReason::Episode(_));
    let mark = if ticked { "[✓] " } else { "[ ] " };
    let body = format!(
        "{}  →  {}",
        display_path(prepared.source(), &plan.root),
        crate::paths::file_name(prepared.destination())
    );
    let room = width.saturating_sub(mark.width() + badge.width() + 2);
    let body = fit(&body, room);
    let gap = width.saturating_sub(mark.width() + body.width() + badge.width());
    Line::from(vec![
        Span::styled(
            mark,
            Style::default().fg(if ticked { theme::TICK } else { theme::FAINT }),
        ),
        Span::styled(body, Style::default().fg(theme::FOREGROUND)),
        Span::raw(" ".repeat(gap)),
        Span::styled(
            badge,
            if certain {
                Style::default().fg(theme::CERTAIN)
            } else {
                theme::faint()
            },
        ),
    ])
}

// ---------------------------------------------------------------- step 4: apply

fn draw_apply(frame: &mut Frame, app: &mut App, area: Rect) {
    let (done, total) = app.progress;
    if app.applying {
        let bar_width = (area.width as usize).saturating_sub(12).max(4);
        let filled = (bar_width * done).checked_div(total).unwrap_or(0);
        let lines = vec![
            Line::from(Span::styled(
                format!("{} Renaming…", SPINNER[app.ticks % SPINNER.len()]),
                Style::default().fg(theme::WORKING),
            )),
            Line::default(),
            Line::from(vec![
                Span::styled("█".repeat(filled), Style::default().fg(theme::SUCCESS)),
                Span::styled(
                    "░".repeat(bar_width - filled),
                    Style::default().fg(theme::BORDER),
                ),
                Span::styled(format!("  {done} / {total}"), theme::muted()),
            ]),
        ];
        frame.render_widget(Paragraph::new(lines), area);
        return;
    }

    let (glyph, colour, headline, detail) = match app.outcome.as_ref() {
        Some(Outcome::Done { applied }) => (
            "✓",
            theme::SUCCESS,
            format!("Renamed {applied} {}", plural(*applied, "file", "files")),
            String::new(),
        ),
        Some(Outcome::Mixed {
            applied,
            failed,
            error,
        }) => (
            "!",
            theme::ERROR,
            format!(
                "Renamed {applied}, {failed} {} failed",
                plural(*failed, "rename", "renames")
            ),
            error.clone(),
        ),
        Some(Outcome::Refused { reason }) => (
            "✗",
            theme::ERROR,
            "Nothing was renamed".into(),
            reason.clone(),
        ),
        None => ("", theme::FAINT, "Nothing to report".into(), String::new()),
    };

    let mut lines = vec![
        Line::from(Span::styled(
            glyph,
            Style::default().fg(colour).add_modifier(Modifier::BOLD),
        ))
        .centered(),
        Line::default(),
        Line::from(Span::styled(headline, Style::default().fg(colour))).centered(),
    ];
    if !detail.is_empty() {
        for line in wrap(&detail, area.width as usize) {
            lines.push(Line::from(Span::styled(line, theme::faint())).centered());
        }
    }
    frame.render_widget(Paragraph::new(lines), area);
    draw_card_buttons(
        frame,
        app,
        Rect::new(area.x, area.bottom() - 1, area.width, 1),
    );
}

// -------------------------------------------------------------- card buttons

#[derive(Clone, Copy, PartialEq, Eq)]
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

/// The same shape inline, for a card row that acts rather than toggles.
fn button_line(label: &str, key: &str) -> Line<'static> {
    Line::from(vec![
        Span::styled(label.to_string(), Style::default().fg(theme::FOREGROUND)),
        Span::styled(format!(" ({key})"), theme::faint()),
    ])
}

/// Columns [`button_spans`] takes, plus the padding a free-standing button gets.
fn button_width(label: &str, key: &str) -> usize {
    label.width() + key.width() + 3 + 2 * BUTTON_PADDING
}

/// Blank columns held either side of a button's text.
const BUTTON_PADDING: usize = 2;

/// The bottom row of a card: back on the left, the way forward on the right.
///
/// The direction of the wizard is the direction of the buttons, so a glance at
/// the card says which way the workflow runs without reading a word.
fn draw_card_buttons(frame: &mut Frame, app: &mut App, area: Rect) {
    if area.height == 0 {
        return;
    }
    let controls = app.controls();
    if let Some(index) = controls.iter().position(|item| *item == Control::Back) {
        let focused = app.focus == index;
        draw_button(
            frame,
            app,
            area,
            false,
            "← Back",
            "esc",
            if focused {
                ButtonKind::Primary
            } else {
                ButtonKind::Neutral
            },
            Hit::Control(Control::Back),
        );
    }

    let (control, label, key, kind) = match app.step {
        Step::Folder => (Control::Advance, "Next →", "↵", ButtonKind::Primary),
        Step::Rules => (Control::Advance, "Preview →", "↵", ButtonKind::Primary),
        Step::Preview => (
            Control::Advance,
            "Apply",
            "a",
            if app.can_apply() {
                ButtonKind::Confirm
            } else {
                ButtonKind::Disabled
            },
        ),
        Step::Apply => (Control::Again, "Start over", "↵", ButtonKind::Primary),
    };
    if !controls.contains(&control) {
        return;
    }
    draw_button(
        frame,
        app,
        area,
        true,
        label,
        key,
        kind,
        Hit::Control(control),
    );
}

#[allow(clippy::too_many_arguments)]
fn draw_button(
    frame: &mut Frame,
    app: &mut App,
    area: Rect,
    right: bool,
    label: &str,
    key: &str,
    kind: ButtonKind,
    hit: Hit,
) {
    let width = button_width(label, key) as u16;
    if width > area.width {
        return;
    }
    let x = if right { area.right() - width } else { area.x };
    let rect = Rect::new(x, area.y, width, 1);
    let under = hovered(app.hover, rect);
    let fill = if under {
        theme::hovered_fill(kind.colours().0)
    } else {
        kind.colours().0
    };
    let pad = Span::styled(" ".repeat(BUTTON_PADDING), Style::default().bg(fill));
    let mut spans = vec![pad.clone()];
    spans.extend(button_spans(label, key, kind, under));
    spans.push(pad);
    frame.render_widget(Paragraph::new(Line::from(spans)), rect);
    app.hits.push((rect, hit));
}

/// A single line of text, centred in both directions.
fn draw_centred(frame: &mut Frame, area: Rect, text: &str, colour: Color) {
    if area.height == 0 {
        return;
    }
    let rect = Rect::new(area.x, area.y + area.height / 2, area.width, 1);
    frame.render_widget(
        Paragraph::new(
            Line::from(Span::styled(text.to_string(), Style::default().fg(colour))).centered(),
        ),
        rect,
    );
}

// ------------------------------------------------------------------- modals

const DIALOG_WIDTH: u16 = 72;

fn draw_help(frame: &mut Frame, area: Rect, hover: Option<Position>, hits: &mut Vec<(Rect, Hit)>) {
    const SHORTCUTS: [(&str, &str); 11] = [
        ("↵", "Forward, from wherever the keyboard is"),
        ("← →   h l", "Back and forward through the four steps"),
        ("↑ ↓   k j", "Move inside the step"),
        ("↹  ⇧↹", "Next / previous control"),
        ("␣", "Press the focused control in place"),
        ("^a  ^r", "Tick everything / nothing"),
        ("g  G", "First / last rename"),
        ("s", "The subtitles that were skipped, and why"),
        ("o", "Browse for a folder"),
        ("a", "Apply the ticked renames"),
        ("?  q", "This list · quit"),
    ];
    let mut lines: Vec<Line> = Vec::new();
    for (keys, meaning) in SHORTCUTS {
        lines.push(Line::from(vec![
            Span::styled(pad(keys, 12), theme::key()),
            Span::styled(meaning.to_string(), theme::muted()),
        ]));
    }
    let height = lines.len() as u16 + 5;
    let rect = centered(area, DIALOG_WIDTH, height);
    render_dialog(
        frame,
        rect,
        " Keyboard shortcuts ",
        Text::from(lines),
        &[(Hit::CloseModal, "Close", "esc", ButtonKind::Primary)],
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
                "Rename {count} {} on disk.",
                plural(count, "subtitle", "subtitles")
            ),
            Style::default().fg(theme::FOREGROUND),
        )),
        Line::default(),
    ];
    for example in examples {
        lines.push(Line::from(Span::styled(
            fit(example, DIALOG_WIDTH as usize - 4),
            theme::muted(),
        )));
    }
    if count > examples.len() {
        lines.push(Line::from(Span::styled(
            format!("… and {} more", count - examples.len()),
            theme::faint(),
        )));
    }
    let height = lines.len() as u16 + 5;
    let rect = centered(area, DIALOG_WIDTH, height);
    render_dialog(
        frame,
        rect,
        " Confirm apply ",
        Text::from(lines),
        &[
            (Hit::ConfirmCancel, "Cancel", "esc", ButtonKind::Neutral),
            (Hit::ConfirmApply, "Apply", "↵", ButtonKind::Confirm),
        ],
        hover,
        hits,
    );
}

/// The skipped subtitles, folded away behind one key until they are wanted.
///
/// A list rather than a paragraph, because a folder can skip more subtitles than
/// a dialog has rows and the reader has to be able to walk them.
fn draw_skipped(frame: &mut Frame, area: Rect, app: &mut App) {
    let Some(preview) = app.preview.as_ref() else {
        return;
    };
    let root = preview.plan.root.clone();
    let count = preview.plan.skipped.len();
    let rect = centered(area, DIALOG_WIDTH + 8, (count as u16 + 6).min(area.height));
    frame.render_widget(Clear, rect);
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme::FOCUS))
        .style(Style::default().bg(theme::PANEL))
        .title_top(Span::styled(
            format!(" Skipped ({count}) "),
            theme::heading(),
        ))
        .padding(Padding::horizontal(1));
    let inner = block.inner(rect);
    frame.render_widget(block, rect);
    if inner.height < 3 {
        return;
    }
    let [list_area, _, buttons_area] = Layout::vertical([
        Constraint::Min(1),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .areas(inner);

    let reason_width = 32.min(list_area.width as usize / 2);
    let path_width = (list_area.width as usize).saturating_sub(reason_width + 2);
    let items: Vec<ListItem> = preview
        .plan
        .skipped
        .iter()
        .map(|skipped| {
            ListItem::new(Line::from(vec![
                Span::styled(
                    pad(
                        &fit(&display_path(&skipped.path, &root), path_width),
                        path_width + 2,
                    ),
                    Style::default().fg(theme::FOREGROUND),
                ),
                Span::styled(
                    fit(&skip_label(&skipped.reason), reason_width),
                    theme::faint(),
                ),
            ]))
        })
        .collect();
    let list = List::new(items).highlight_style(
        Style::default()
            .bg(theme::SELECTION_BACKGROUND)
            .add_modifier(Modifier::BOLD),
    );
    let hover = app.hover;
    let Some(Modal::Skipped(state)) = app.modal.as_mut() else {
        return;
    };
    frame.render_stateful_widget(list, list_area, state);
    let offset = state.offset();
    for visible in 0..list_area.height as usize {
        if offset + visible >= count {
            break;
        }
        app.hits.push((
            Rect::new(
                list_area.x,
                list_area.y + visible as u16,
                list_area.width,
                1,
            ),
            Hit::Row(offset + visible),
        ));
    }
    draw_dialog_buttons(
        frame,
        buttons_area,
        &[(Hit::CloseModal, "Close", "esc", ButtonKind::Primary)],
        hover,
        &mut app.hits,
    );
}

fn draw_picker(
    frame: &mut Frame,
    area: Rect,
    picker: &mut crate::tui::picker::Picker,
    hover: Option<Position>,
    hits: &mut Vec<(Rect, Hit)>,
) {
    let rect = centered(area, DIALOG_WIDTH, 18);
    frame.render_widget(Clear, rect);
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme::FOCUS))
        .style(Style::default().bg(theme::PANEL))
        .title_top(Span::styled(" Browse ", theme::heading()))
        .padding(Padding::horizontal(1));
    let inner = block.inner(rect);
    frame.render_widget(block, rect);
    let [path_area, list_area, _, buttons_area] = Layout::vertical([
        Constraint::Length(2),
        Constraint::Min(1),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .areas(inner);

    frame.render_widget(
        Paragraph::new(vec![
            Line::from(Span::styled(
                fit(&picker.current.to_string_lossy(), path_area.width as usize),
                Style::default().fg(theme::FOREGROUND),
            )),
            Line::default(),
        ]),
        path_area,
    );

    if let Some(error) = picker.error.clone() {
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                error,
                Style::default().fg(theme::ERROR),
            )))
            .wrap(Wrap { trim: false }),
            list_area,
        );
    } else {
        let items: Vec<ListItem> = picker
            .entries
            .iter()
            .map(|entry| {
                ListItem::new(Line::from(Span::styled(
                    format!("  {}/", crate::paths::file_name(entry)),
                    theme::muted(),
                )))
            })
            .collect();
        let empty = items.is_empty();
        let list = List::new(items).highlight_style(
            Style::default()
                .bg(theme::SELECTION_BACKGROUND)
                .add_modifier(Modifier::BOLD),
        );
        frame.render_stateful_widget(list, list_area, &mut picker.state);
        if empty {
            frame.render_widget(
                Paragraph::new(Line::from(Span::styled(
                    "No subfolders here",
                    theme::faint(),
                ))),
                list_area,
            );
        }
        let offset = picker.state.offset();
        for visible in 0..list_area.height as usize {
            let index = offset + visible;
            if index >= picker.entries.len() {
                break;
            }
            hits.push((
                Rect::new(
                    list_area.x,
                    list_area.y + visible as u16,
                    list_area.width,
                    1,
                ),
                Hit::PickerRow(index),
            ));
        }
    }

    draw_dialog_buttons(
        frame,
        buttons_area,
        &[
            (Hit::PickerCancel, "Cancel", "esc", ButtonKind::Neutral),
            (Hit::PickerParent, "Parent", "←", ButtonKind::Neutral),
            (Hit::PickerUse, "Use this folder", "s", ButtonKind::Primary),
        ],
        hover,
        hits,
    );
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
    // time; the primary action always stays, so a mouse-only user is never
    // stranded inside a dialog.
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
    fn fit_keeps_the_filename_end() {
        assert_eq!(fit("abcdef", 10), "abcdef");
        assert_eq!(fit("abcdef", 4), "…def");
    }

    #[test]
    fn pad_fills_to_the_width() {
        assert_eq!(pad("ab", 5), "ab   ");
        assert_eq!(pad("abcdef", 3), "abcdef");
    }

    #[test]
    fn wrap_breaks_on_words() {
        assert_eq!(wrap("one two three", 12), vec!["one two", "three"]);
    }
}
