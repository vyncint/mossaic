//! The pixel-art editor: draw on the year directly, and save what you drew.
//!
//! The model here holds no terminal and does no drawing, so every key and every
//! click is a plain function from state to state. That is what makes the
//! editor testable at all: a TUI checked only through a PTY can be asserted on
//! what it *renders*, and the interesting claims — that undo restores exactly
//! what was there, that a drag paints every cell it crosses, that the estimate
//! matches what `--write` would actually commit — are about state rather than
//! about pixels.
//!
//! [`render`] is the other half, and the only part that needs a screen.

use std::path::{Path, PathBuf};

use chrono::NaiveDate;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use crate::art::{self, Canvas, Grid, CANVAS_ROWS};
use crate::primer::{Appearance, Legibility, Palette, Season};
use crate::thousands;

/// How deep the undo stack goes.
///
/// A canvas is 371 bytes, so a hundred of them is a rounding error in memory
/// and rather more mistakes than anyone makes in one sitting.
const UNDO_DEPTH: usize = 128;

/// What a keystroke asked the editor to do about the outside world.
///
/// Returned rather than done, so the model never touches the filesystem: a test
/// can press `s` and assert that a save was asked for without a file appearing
/// anywhere.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// Nothing that concerns the caller.
    Idle,
    /// Write the canvas to the output path.
    Save,
    /// Leave.
    Quit,
}

/// The editor's whole state.
#[derive(Debug, Clone)]
pub struct Editor {
    /// The picture being drawn.
    pub canvas: Canvas,
    /// The year it is being drawn on, which is what gives a cell a date.
    pub grid: Grid,
    /// The calendar column the canvas's first column sits on.
    pub start: usize,
    /// Where the cursor is, as `(week, row)` **within the canvas**.
    pub cursor: (usize, usize),
    /// The level the next paint applies. Set by the digit keys and by a click.
    pub brush: u8,
    /// Whether a mouse button is down, so a drag paints as it crosses.
    pub painting: bool,
    /// Canvas snapshots, oldest first.
    undo: Vec<Canvas>,
    /// Whether anything has changed since the last save.
    pub dirty: bool,
    /// Where `s` writes.
    pub output: PathBuf,
    /// The last thing worth telling the person drawing.
    pub status: String,
    /// Whether the help overlay is up.
    pub help: bool,
    /// What a level-4 day is priced at, for the estimate.
    pub commits: u32,
}

impl Editor {
    /// A new editor over `canvas`, drawing on `grid`.
    #[must_use]
    pub fn new(canvas: Canvas, grid: Grid, start: usize, output: PathBuf, commits: u32) -> Self {
        Self {
            canvas,
            grid,
            start,
            cursor: (0, 0),
            brush: 4,
            painting: false,
            undo: Vec::new(),
            dirty: false,
            output,
            status: String::new(),
            help: false,
            commits: commits.max(1),
        }
    }

    /// The date the cursor is on, or `None` when it falls outside the year.
    ///
    /// Outside is ordinary rather than exceptional: the first and last calendar
    /// columns are partial weeks, so the corners of a full-width canvas are
    /// days the year does not have.
    #[must_use]
    pub fn date_at(&self, week: usize, row: usize) -> Option<NaiveDate> {
        let column = self.start.checked_add(week)?;
        if column >= self.grid.weeks || row >= CANVAS_ROWS {
            return None;
        }
        let date = self.grid.date_at(column, row);
        self.grid.holds(date).then_some(date)
    }

    /// The date under the cursor.
    #[must_use]
    pub fn cursor_date(&self) -> Option<NaiveDate> {
        self.date_at(self.cursor.0, self.cursor.1)
    }

    /// Take a snapshot, so the next change can be undone.
    ///
    /// Called before a change rather than after it, and only when something is
    /// actually about to differ — pressing `3` on a cell that is already 3 must
    /// not consume an undo step, or a stuck key eats the history.
    fn remember(&mut self) {
        if self.undo.len() == UNDO_DEPTH {
            self.undo.remove(0);
        }
        self.undo.push(self.canvas.clone());
    }

    /// Paint the cursor's cell, if it is not already that level.
    pub fn paint(&mut self, level: u8) {
        let (week, row) = self.cursor;
        if self.canvas.at(week, row) == level.min(4) {
            return;
        }
        self.remember();
        self.canvas.set(week, row, level);
        self.dirty = true;
    }

    /// Step the cursor, stopping at the edges rather than wrapping.
    ///
    /// Stopping rather than wrapping because the canvas is a *picture*: running
    /// off the right edge and reappearing on the left, seven rows up, is never
    /// what the hand meant.
    pub fn move_by(&mut self, dx: isize, dy: isize) {
        let width = self.canvas.width().max(1);
        let (week, row) = self.cursor;
        let next_week = (week as isize + dx).clamp(0, width as isize - 1) as usize;
        let next_row = (row as isize + dy).clamp(0, CANVAS_ROWS as isize - 1) as usize;
        self.cursor = (next_week, next_row);
    }

    /// Fill the whole canvas with one level.
    pub fn fill(&mut self, level: u8) {
        self.remember();
        for week in 0..self.canvas.width() {
            for row in 0..CANVAS_ROWS {
                self.canvas.set(week, row, level);
            }
        }
        self.dirty = true;
        self.status = format!("filled with level {}", level.min(4));
    }

    /// Turn every shade over: 0 becomes 4, 1 becomes 3, and so on.
    pub fn invert(&mut self) {
        self.remember();
        for week in 0..self.canvas.width() {
            for row in 0..CANVAS_ROWS {
                let level = self.canvas.at(week, row);
                self.canvas.set(week, row, 4u8.saturating_sub(level));
            }
        }
        self.dirty = true;
        self.status = "inverted".to_string();
    }

    /// Step back one change, and say whether there was one.
    pub fn undo(&mut self) -> bool {
        match self.undo.pop() {
            Some(previous) => {
                self.canvas = previous;
                self.dirty = true;
                self.status = format!("undone — {} step(s) left", self.undo.len());
                true
            }
            None => {
                self.status = "nothing to undo".to_string();
                false
            }
        }
    }

    /// How many undo steps are held.
    #[must_use]
    pub fn undo_depth(&self) -> usize {
        self.undo.len()
    }

    /// The busiest day the picture needs, which sets every other price.
    #[must_use]
    pub fn peak(&self) -> u32 {
        self.commits.max(self.canvas.min_peak())
    }

    /// What drawing this would cost, in commits.
    ///
    /// The same arithmetic `--write` uses, so the number on screen is the
    /// number you would be asked to make rather than an approximation of it.
    #[must_use]
    pub fn estimate(&self) -> u32 {
        let peak = self.peak();
        (0..self.canvas.width())
            .flat_map(|week| (0..CANVAS_ROWS).map(move |row| (week, row)))
            .filter(|(week, row)| self.date_at(*week, *row).is_some())
            .map(|(week, row)| self.canvas.at(week, row))
            .filter(|level| *level > 0)
            .fold(0u32, |sum, level| {
                sum.saturating_add(art::commits_to_reach(level, peak))
            })
    }

    /// The two shades in the drawing that look most alike, and how far apart
    /// they are in the worst palette a reader might have.
    ///
    /// The closest pair rather than the widest, because the widest always
    /// flatters: a drawing spanning levels 0 to 4 reads as ΔE 70 and can still
    /// hold two shades nobody can separate. See [`art::Canvas::closest_pair`].
    #[must_use]
    pub fn legibility(&self) -> Option<(u8, u8, Legibility, f32)> {
        self.canvas.closest_pair()
    }

    /// Handle a keystroke.
    pub fn on_key(&mut self, code: ratatui::crossterm::event::KeyCode, ctrl: bool) -> Outcome {
        use ratatui::crossterm::event::KeyCode::*;

        if self.help && !matches!(code, Char('?') | Esc | Char('q')) {
            // Any other key closes the overlay and is then acted on, so the
            // help never eats the keystroke that dismissed it.
            self.help = false;
        }

        match code {
            Char('q') | Esc if self.help => {
                self.help = false;
                Outcome::Idle
            }
            Char('?') => {
                self.help = !self.help;
                Outcome::Idle
            }
            Char('q') | Esc => Outcome::Quit,
            Char('c') if ctrl => Outcome::Quit,
            Char('z') if ctrl => {
                self.undo();
                Outcome::Idle
            }

            Left | Char('h') => {
                self.move_by(-1, 0);
                Outcome::Idle
            }
            Right | Char('l') => {
                self.move_by(1, 0);
                Outcome::Idle
            }
            Up | Char('k') => {
                self.move_by(0, -1);
                Outcome::Idle
            }
            Down | Char('j') => {
                self.move_by(0, 1);
                Outcome::Idle
            }
            Home => {
                self.cursor.0 = 0;
                Outcome::Idle
            }
            End => {
                self.cursor.0 = self.canvas.width().saturating_sub(1);
                Outcome::Idle
            }

            Char(digit @ '0'..='4') => {
                let level = digit as u8 - b'0';
                self.brush = level;
                self.paint(level);
                Outcome::Idle
            }
            Char(' ') | Enter => {
                let (week, row) = self.cursor;
                let next = (self.canvas.at(week, row) + 1) % 5;
                self.brush = next;
                self.paint(next);
                Outcome::Idle
            }

            Char('c') => {
                self.fill(0);
                Outcome::Idle
            }
            Char('i') => {
                self.invert();
                Outcome::Idle
            }
            Char('u') => {
                self.undo();
                Outcome::Idle
            }
            Char('s') => Outcome::Save,
            _ => Outcome::Idle,
        }
    }

    /// Handle a mouse event, given where the canvas was drawn on screen.
    ///
    /// `origin` is the top-left cell of the grid as it was rendered. The model
    /// is told where it was drawn rather than deciding: layout belongs to
    /// [`render`], and duplicating it here is how the two drift apart.
    pub fn on_mouse(&mut self, event: ratatui::crossterm::event::MouseEvent, origin: Rect) {
        use ratatui::crossterm::event::{MouseButton, MouseEventKind};

        let inside = |column: u16, row: u16| -> Option<(usize, usize)> {
            let week = column.checked_sub(origin.x)? as usize;
            let cell_row = row.checked_sub(origin.y)? as usize;
            (week < self.canvas.width() && cell_row < CANVAS_ROWS).then_some((week, cell_row))
        };

        match event.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                if let Some(cell) = inside(event.column, event.row) {
                    self.cursor = cell;
                    self.painting = true;
                    self.paint(self.brush);
                }
            }
            MouseEventKind::Drag(MouseButton::Left) => {
                if self.painting {
                    if let Some(cell) = inside(event.column, event.row) {
                        self.cursor = cell;
                        self.paint(self.brush);
                    }
                }
            }
            MouseEventKind::Up(_) => self.painting = false,
            MouseEventKind::Moved => {
                // Hover moves the cursor without painting, so the date and the
                // level under the pointer are readable before committing to a
                // stroke.
                if let Some(cell) = inside(event.column, event.row) {
                    self.cursor = cell;
                }
            }
            MouseEventKind::Down(MouseButton::Right) => {
                if let Some(cell) = inside(event.column, event.row) {
                    self.cursor = cell;
                    self.paint(0);
                }
            }
            _ => {}
        }
    }

    /// Write the canvas to its output path.
    pub fn save(&mut self) -> Result<(), String> {
        let body = self.canvas.to_art();
        if let Some(parent) = self
            .output
            .parent()
            .filter(|path| !path.as_os_str().is_empty())
        {
            std::fs::create_dir_all(parent)
                .map_err(|error| format!("could not make {}: {error}", parent.display()))?;
        }
        std::fs::write(&self.output, body)
            .map_err(|error| format!("could not write {}: {error}", self.output.display()))?;
        self.dirty = false;
        self.status = format!("saved {}", self.output.display());
        Ok(())
    }
}

/// The width of the row-label gutter, which is also where the grid starts.
const GUTTER: u16 = 5;

/// Where the canvas grid lands inside `area`, so the model and the mouse agree
/// about which cell is under the pointer.
///
/// One function, used by both [`render`] and the caller that feeds mouse
/// events to [`Editor::on_mouse`]. Working the offset out twice is exactly how
/// a click lands one row above where it was aimed.
///
/// The `+ 1` is the month ruler drawn above the first weekday row; the gutter
/// is the `Sun `-width row label.
#[must_use]
pub fn grid_origin(area: Rect) -> Rect {
    Rect {
        x: area.x + GUTTER,
        y: area.y + 1,
        width: area.width.saturating_sub(GUTTER),
        height: CANVAS_ROWS as u16,
    }
}

/// Draw the editor.
pub fn render(frame: &mut Frame<'_>, editor: &Editor, palette: &Palette) {
    let area = frame.area();
    let [top, body, hud, keys] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(CANVAS_ROWS as u16 + 2),
        Constraint::Min(6),
        Constraint::Length(1),
    ])
    .areas(area);

    let name = editor
        .canvas
        .meta()
        .name
        .clone()
        .unwrap_or_else(|| "untitled".to_string());
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                format!(" {name} "),
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::raw(format!(
                "{}x{}  ·  {}  ·  {}{}",
                editor.canvas.width(),
                CANVAS_ROWS,
                editor.grid.year,
                editor.output.display(),
                if editor.dirty { " *" } else { "" }
            )),
        ])),
        top,
    );

    // The grid. One character per day, because 53 columns at two characters
    // each is 106 and does not fit the terminal most people have.
    const NAMES: [&str; CANVAS_ROWS] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
    let mut lines: Vec<Line<'_>> = Vec::with_capacity(CANVAS_ROWS + 2);

    // A month ruler, so a column is locatable in the year rather than only in
    // the picture.
    let mut ruler = String::from("     ");
    let mut last_month = 0;
    for week in 0..editor.canvas.width() {
        let label = match editor.date_at(week, 3) {
            Some(date) => {
                use chrono::Datelike;
                if date.month() != last_month {
                    last_month = date.month();
                    const MONTHS: [&str; 12] =
                        ["J", "F", "M", "A", "M", "J", "J", "A", "S", "O", "N", "D"];
                    MONTHS[(date.month() - 1) as usize]
                } else {
                    " "
                }
            }
            None => " ",
        };
        ruler.push_str(label);
    }
    lines.push(Line::from(Span::styled(
        ruler,
        Style::default().fg(Color::DarkGray),
    )));

    for (row, name) in NAMES.iter().enumerate() {
        let mut spans = vec![Span::styled(
            format!("{name:<4} "),
            Style::default().fg(Color::DarkGray),
        )];
        for week in 0..editor.canvas.width() {
            let level = editor.canvas.at(week, row);
            let rgb = palette.levels[usize::from(level).min(4)];
            let outside = editor.date_at(week, row).is_none();
            let here = editor.cursor == (week, row);
            let glyph = if outside { '·' } else { '█' };
            let mut style = Style::default().fg(Color::Rgb(rgb.0, rgb.1, rgb.2));
            if outside {
                style = Style::default().fg(Color::DarkGray);
            }
            if here {
                style = style.add_modifier(Modifier::REVERSED);
            }
            spans.push(Span::styled(glyph.to_string(), style));
        }
        lines.push(Line::from(spans));
    }
    frame.render_widget(Paragraph::new(lines), body);

    // The numbers, which are the reason to draw here rather than in an editor.
    let histogram = editor.canvas.histogram();
    let peak = editor.peak();
    let mut stats: Vec<Line<'_>> = Vec::new();
    stats.push(Line::from(match editor.cursor_date() {
        Some(date) => format!(
            "  {}  ·  level {}  ·  brush {}",
            date.format("%a %-d %b %Y"),
            editor.canvas.at(editor.cursor.0, editor.cursor.1),
            editor.brush
        ),
        None => format!(
            "  outside {} — the first and last columns are partial weeks  ·  brush {}",
            editor.grid.year, editor.brush
        ),
    }));
    stats.push(Line::from(""));
    for level in (0..=4u8).rev() {
        let count = histogram[usize::from(level)];
        let rgb = palette.levels[usize::from(level)];
        let bar = "█".repeat(count * 30 / (editor.canvas.width() * CANVAS_ROWS).max(1));
        stats.push(Line::from(vec![
            Span::raw(format!("  level {level}  {count:>4} day(s)  ")),
            Span::styled(bar, Style::default().fg(Color::Rgb(rgb.0, rgb.1, rgb.2))),
            Span::raw(if level == 0 {
                "  (must stay dark)".to_string()
            } else {
                format!(
                    "  {} commits each",
                    thousands(art::commits_to_reach(level, peak))
                )
            }),
        ]));
    }
    stats.push(Line::from(""));
    stats.push(Line::from(format!(
        "  {} commits in total, at --commits {}",
        thousands(editor.estimate()),
        editor.commits
    )));
    match editor.legibility() {
        Some((low, high, legibility, delta)) => stats.push(Line::from(format!(
            "  closest shades {low} and {high} read as {legibility} — ΔE {delta:.0} at worst"
        ))),
        None => stats.push(Line::from(
            "  one shade only — nothing to tell apart yet".to_string(),
        )),
    }
    if !editor.status.is_empty() {
        stats.push(Line::from(""));
        stats.push(Line::from(Span::styled(
            format!("  {}", editor.status),
            Style::default().fg(Color::Yellow),
        )));
    }
    frame.render_widget(
        Paragraph::new(stats).block(Block::default().borders(Borders::TOP)),
        hud,
    );

    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            " arrows/hjkl move · 0-4 paint · space cycle · c clear · i invert · u undo · s save · ? help · q quit",
            Style::default().fg(Color::DarkGray),
        ))),
        keys,
    );

    if editor.help {
        render_help(frame, area);
    }
}

/// The help overlay.
fn render_help(frame: &mut Frame<'_>, area: Rect) {
    let lines = vec![
        Line::from(Span::styled(
            " Drawing on the year ",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from("  arrows, or h j k l   move the cursor"),
        Line::from("  Home / End           first and last column"),
        Line::from("  0 1 2 3 4            paint that shade, and take it as the brush"),
        Line::from("  space, enter         cycle this cell 0 to 4 and round again"),
        Line::from("  click, drag          paint with the brush; right-click clears"),
        Line::from("  move the pointer     read a day without painting it"),
        Line::from(""),
        Line::from("  c                    clear the whole canvas"),
        Line::from("  i                    invert every shade"),
        Line::from("  u, ctrl-z            undo"),
        Line::from("  s                    save to the output file"),
        Line::from(""),
        Line::from("  ?                    close this"),
        Line::from("  q, esc               leave"),
        Line::from(""),
        Line::from(Span::styled(
            "  Level 0 is a day that must stay dark. The commit counts on the",
            Style::default().fg(Color::DarkGray),
        )),
        Line::from(Span::styled(
            "  left are what --write would actually make, not an estimate.",
            Style::default().fg(Color::DarkGray),
        )),
    ];
    let height = lines.len() as u16 + 2;
    let width = 70.min(area.width);
    let box_area = Rect {
        x: area.x + (area.width.saturating_sub(width)) / 2,
        y: area.y + (area.height.saturating_sub(height)) / 2,
        width,
        height: height.min(area.height),
    };
    frame.render_widget(ratatui::widgets::Clear, box_area);
    frame.render_widget(
        Paragraph::new(lines).block(Block::default().borders(Borders::ALL).title(" help ")),
        box_area,
    );
}

/// The palette the editor paints with.
#[must_use]
pub fn palette() -> Palette {
    Palette::new(Appearance::Dark, Season::Default, true)
}

/// Where a drawing goes when nobody says.
#[must_use]
pub fn default_output(name: Option<&str>) -> PathBuf {
    Path::new(&format!("{}.art", name.unwrap_or("drawing"))).to_path_buf()
}
