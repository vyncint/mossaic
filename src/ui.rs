//! Rendering. The whole screen is one paragraph of pre-built lines, so the chart
//! never needs layout maths beyond choosing a cell style that fits — and when the
//! terminal draws pixels, the seven weekday rows are left blank for
//! [`crate::graphics`] to paint into, which is why [`Layout`] travels back out to
//! the app: the painter and the mouse both need to know where the grid landed.

use chrono::{Datelike, Local, NaiveDate};
use ratatui::layout::Rect;
use ratatui::style::{Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Paragraph};
use ratatui::Frame;

use crate::app::{App, CellStyle, Load, Mode};
use crate::calendar::{Calendar, Day};
use crate::graphics::COLUMNS_PER_DAY;
use crate::primer::Palette;
use crate::thousands;

/// Width of the weekday label column, e.g. "Mon ".
const GUTTER: usize = 4;
/// Only alternating rows are labelled, the way GitHub does it.
const WEEKDAYS: [&str; 7] = ["", "Mon", "", "Wed", "", "Fri", ""];
const SPINNER: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
/// Fills a day that has happened.
const FILL: &str = "█";
/// Marks the cursor. A partial shade lets the day's own level show through.
const CURSOR: &str = "▓";
/// A half-height cell. A character is about twice as tall as it is wide, so half of
/// one is square, and the unpainted lower half becomes the gap below it.
const SQUARE: &str = "▀";
/// A rounded square, two characters wide. These are block sextants: each character
/// is a 2×3 grid of sub-blocks, so the pair is 4×3 and can have all four corners
/// shaved — the finest rounding a character grid allows.
///
/// ```text
///   U+1FB2B  U+1FB1B      .## .            .##.
///     .#       #.          ###  #   =>      ####
///     ##       ##          .## .            .##.
///     .#       #.
/// ```
const ROUND: &str = "\u{1FB2B}\u{1FB1B}";
/// Rows the chart needs besides the cells: header, blank, months, blank, detail,
/// legend, summary, blank, footer — and the note under the legend, which
/// `chart` adds whenever it has something to say.
///
/// Counting the note is what keeps the footer on screen. `fits` budgeted nine and
/// `chart` could push ten, so at a height of exactly `cells.height() + 11` the
/// note evicted the only line that lists the keys. Budgeting it costs a slightly
/// smaller style at that one height, which is the better of the two failures.
pub(crate) const CHROME_ROWS: usize = 10;

/// Rows and columns the bordered block in [`draw`] takes before anything is
/// drawn inside it — one on each side, both ways.
///
/// Everything below works in that inner area, which is the right frame for
/// laying out a chart and the wrong one for advice. `note` told a reader with a
/// 17-row window that it "has 15", so resizing to the 17 it asked for still did
/// not fit. The number a reader can act on is the one their terminal reports.
pub(crate) const BORDER: usize = 2;

/// A concrete cell layout, resolved from a [`CellStyle`] and the space available.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cells {
    /// Actual pixels, drawn by the terminal's graphics protocol: rounded exactly,
    /// square whatever the font, and in Primer's own greens. Two columns and one
    /// row per day, the same stride as [`Cells::Squares`], so everything around it
    /// stays text.
    Pixels,
    /// Small filled squares separated in both directions and drawn with no outline
    /// at all — how github.com actually looks. Each day is half a character tall,
    /// which is square, followed by a blank column.
    Squares,
    /// Box-bordered cells, `fill` columns of content each. A terminal character is
    /// about twice as tall as it is wide, so `fill: 2` reads as a square and
    /// `fill: 1` as a tall rectangle.
    Grid {
        /// Columns of content per cell; 2 reads as a square.
        fill: usize,
    },
    /// Genuinely rounded squares: a sextant pair per day, so the corners are
    /// shaved at sub-character resolution. The closest match to github.com short of
    /// pixels, but it needs the terminal to draw U+1FB00 block sextants.
    /// Without the `gap` column the cells butt together and their middle rows join
    /// into a continuous bar — rounded, but reading as a chain rather than as
    /// separate squares. It buys back a third of the width.
    Rounded {
        /// Blank columns after each cell. Zero makes them touch.
        gap: usize,
    },
    /// Bare cells: `fill` coloured columns then `gap` blank ones.
    Solid {
        /// Coloured columns per cell.
        fill: usize,
        /// Blank columns after them.
        gap: usize,
    },
}

impl Cells {
    /// Columns consumed per week, which is also the month-label stride.
    pub const fn stride(&self) -> usize {
        match self {
            Self::Pixels => COLUMNS_PER_DAY as usize,
            // One content column plus the gap beside it.
            Self::Squares => 2,
            // Two characters of rounded cell, plus whatever gap follows it.
            Self::Rounded { gap } => 2 + *gap,
            // One shared border column per cell.
            Self::Grid { fill } => *fill + 1,
            Self::Solid { fill, gap } => *fill + *gap,
        }
    }

    /// Columns before the first cell, past the gutter. The grid has a left border.
    const fn offset(&self) -> usize {
        match self {
            Self::Grid { .. } => 1,
            Self::Pixels | Self::Rounded { .. } | Self::Squares | Self::Solid { .. } => 0,
        }
    }

    /// Columns the whole chart needs, gutter included.
    pub const fn width(&self, weeks: usize) -> usize {
        match self {
            // Borders are shared between neighbours, plus one to close the right edge.
            Self::Grid { .. } => GUTTER + weeks * self.stride() + 1,
            Self::Pixels | Self::Rounded { .. } | Self::Squares | Self::Solid { .. } => {
                GUTTER + weeks * self.stride()
            }
        }
    }

    /// Rows the cells themselves occupy: seven weekdays, plus rules for the grid.
    pub const fn height(&self) -> usize {
        match self {
            Self::Grid { .. } => 7 + 6 + 2,
            Self::Pixels | Self::Rounded { .. } | Self::Squares | Self::Solid { .. } => 7,
        }
    }

    /// The legend chip: one cell drawn exactly as the chart draws it. In pixel mode
    /// the swatches are an image too, so the text only reserves the room.
    fn swatch(&self) -> String {
        match self {
            Self::Pixels => " ".repeat(COLUMNS_PER_DAY as usize),
            Self::Rounded { .. } => ROUND.to_string(),
            Self::Squares => SQUARE.to_string(),
            Self::Grid { fill } | Self::Solid { fill, .. } => FILL.repeat(*fill),
        }
    }

    /// Whether the legend puts a space between swatches. Pixel swatches carry their
    /// own gap, exactly as the chart's cells do.
    const fn swatch_gap(&self) -> usize {
        match self {
            Self::Pixels => 0,
            _ => 1,
        }
    }

    /// The name shown beside the legend.
    pub const fn name(&self) -> &'static str {
        match self {
            Self::Pixels => "pixel",
            Self::Rounded { gap: 0 } => "snug",
            Self::Rounded { .. } => "rounded",
            Self::Squares => "squares",
            Self::Grid { fill: 2 } => "grid",
            Self::Grid { .. } => "slim",
            Self::Solid { fill: 2, gap: 1 } => "spaced",
            Self::Solid { fill: 2, .. } => "blocks",
            Self::Solid { .. } => "compact",
        }
    }

    const fn fits(&self, weeks: usize, width: usize, height: usize) -> bool {
        self.width(weeks) <= width && self.height() + CHROME_ROWS <= height
    }
}

/// Where the grid ended up on screen, so the painter can put pixels on it and the
/// mouse can tell which day it is over. Screen coordinates, not frame-relative.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Layout {
    /// Column of the first cell, and row of the first cell row.
    pub x: u16,
    /// Row of the first cell row.
    pub y: u16,
    /// Columns in the grid.
    pub weeks: u16,
    /// The style the chart resolved to.
    pub cells: Cells,
    /// Where the legend swatches start, when the legend is on screen.
    pub legend: Option<(u16, u16)>,
    /// The row past the bottom of the drawable area. An image is placed at the
    /// cursor and drawn downwards from there, so one that would reach past the last
    /// row scrolls the screen out from under everything else — worth one comparison
    /// to rule out, since `d` can ask for cells the terminal has no room for.
    pub bottom: u16,
    /// The column past the right edge, for exactly the same reason. Sixel clears
    /// the character cells it is about to cover, and it did that by writing spaces
    /// with no idea how wide the terminal was: a 53-week grid wrote 106 of them
    /// from column 6, so on an 80-column terminal seven rows wrapped over the
    /// gutter, the border and the rows below — and ratatui, believing it had
    /// written those cells, never repainted them.
    pub right: u16,
}

impl Layout {
    /// Whether the cells fit on the screen from where they start.
    pub fn has_room(&self) -> bool {
        self.y + self.cells.height() as u16 <= self.bottom
            && self.x + self.weeks * COLUMNS_PER_DAY <= self.right
    }

    /// Which day a character cell belongs to. Deliberately forgiving: a click on the
    /// gap beside a cell, or on a grid rule, snaps to the day it borders rather than
    /// falling through, because a two-column target is small enough already.
    pub fn hit(&self, column: u16, row: u16) -> Option<(usize, usize)> {
        let dx = usize::from(column.checked_sub(self.x)?);
        let dy = usize::from(row.checked_sub(self.y)?);
        let week = dx / self.cells.stride();
        // The bordered styles spend a row on a rule between every two weekdays.
        let weekday = match self.cells {
            Cells::Grid { .. } => dy.saturating_sub(1) / 2,
            _ => dy,
        };
        (week < usize::from(self.weeks) && weekday < 7 && dy < self.cells.height())
            .then_some((week, weekday))
    }
}

/// Honour an explicit style even when it will clip; for `Auto`, take the most
/// faithful one the terminal can show in full. `pixels` is whether the terminal
/// answered yes to a graphics protocol.
pub fn resolve(style: CellStyle, weeks: usize, width: usize, height: usize, pixels: bool) -> Cells {
    const SQUARES: Cells = Cells::Squares;
    const ROUNDED: Cells = Cells::Rounded { gap: 1 };
    const SNUG: Cells = Cells::Rounded { gap: 0 };
    const GRID: Cells = Cells::Grid { fill: 2 };
    const SLIM: Cells = Cells::Grid { fill: 1 };
    const SPACED: Cells = Cells::Solid { fill: 2, gap: 1 };
    const BLOCKS: Cells = Cells::Solid { fill: 2, gap: 0 };
    const COMPACT: Cells = Cells::Solid { fill: 1, gap: 0 };

    match style {
        // Asking for pixels on a terminal that cannot draw them falls to the closest
        // thing that is still rounded, rather than to nothing.
        CellStyle::Pixels if pixels => Cells::Pixels,
        CellStyle::Pixels => ROUNDED,
        CellStyle::Squares => SQUARES,
        CellStyle::Rounded => ROUNDED,
        CellStyle::Snug => SNUG,
        CellStyle::Grid => GRID,
        CellStyle::Slim => SLIM,
        CellStyle::Spaced => SPACED,
        CellStyle::Blocks => BLOCKS,
        CellStyle::Compact => COMPACT,
        // Pixels first when the terminal has them: they are the only style that is
        // rounded, square and gapped at once. Then rounded, which gives up the gap
        // between rows; then squares, which gives up the rounding but needs less
        // width. Only fall to the narrow ones when nothing wider fits, since a
        // one-column cell reads as a rectangle.
        CellStyle::Auto => [
            Cells::Pixels,
            ROUNDED,
            SQUARES,
            GRID,
            SPACED,
            BLOCKS,
            SLIM,
            COMPACT,
        ]
        .into_iter()
        .filter(|cells| pixels || *cells != Cells::Pixels)
        .find(|cells| cells.fits(weeks, width, height))
        .unwrap_or(COMPACT),
    }
}

/// Draw the whole chart, and record where the grid landed so the painter and
/// the mouse can find it.
pub fn draw(frame: &mut Frame<'_>, app: &mut App) {
    let area = frame.area();
    let block = Block::bordered()
        .border_style(Style::new().fg(app.palette.ansi(app.palette.rule)))
        .title(Line::from(" mossaic ".bold()));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    app.footer_width = inner.width;
    let (lines, layout) = body(app, inner);
    frame.render_widget(Paragraph::new(lines), inner);
    app.layout = layout;

    if let Some(layout) = layout {
        tooltip(frame, app, &layout, inner);
    }
    if app.help {
        help(frame, app, inner);
    }
}

/// Everything a first run needs to know, including the one thing only the
/// program can answer: what this terminal turned out to be able to draw.
fn help(frame: &mut Frame<'_>, app: &App, inner: Rect) {
    let palette = &app.palette;
    let key = Style::new().fg(palette.ansi(palette.fg)).bold();
    let text = Style::new().fg(palette.ansi(palette.fg));
    let muted = Style::new().fg(palette.ansi(palette.muted));

    let cells = app.layout.map_or("—", |layout| layout.cells.name());
    let yes_no = |flag: bool| if flag { "yes" } else { "no" };

    // Every line with what it is worth, because a short terminal cannot have all
    // of them and truncating from the bottom dropped the most valuable ones: the
    // answer about this terminal, and how to close the panel. Higher survives.
    const SPACER: u8 = 0;
    const HINT: u8 = 1;
    const HEADING: u8 = 2;
    const KEY: u8 = 3;
    const FACT: u8 = 4;
    const DECISION: u8 = 5;
    const WAY_OUT: u8 = 6;

    let heading = |title: &'static str| (HEADING, Line::styled(title, muted));
    let row = |keys: &'static str, what: &'static str| {
        (
            KEY,
            Line::from(vec![
                Span::styled(format!("  {keys:<16}"), key),
                Span::styled(what, text),
            ]),
        )
    };
    let fact = |rank: u8, name: &'static str, value: String| {
        (
            rank,
            Line::from(vec![
                Span::styled(format!("  {name:<16}"), muted),
                Span::styled(value, text),
            ]),
        )
    };
    let blank = || (SPACER, Line::raw(""));

    let mut lines = vec![
        heading(" Moving"),
        row("← → / h l", "previous / next week"),
        row("↑ ↓ / k j", "previous / next day"),
        row("[ ] · PgUp PgDn", "previous / next year"),
        row("t · Home End", "today · first / last day"),
        blank(),
        heading(" Mouse"),
        row("hover a day", "its tooltip, as github.com writes it"),
        row("click", "move the cursor there"),
        row("wheel", "previous / next year"),
        row("m", "mouse reporting off / on"),
        blank(),
        heading(" Chart"),
        row("d", "cycle cell style"),
        row("u · r", "another user · reload"),
        row("q", "quit"),
        blank(),
        heading(" This terminal"),
        fact(FACT, "kitty graphics", yes_no(app.caps.kitty).to_string()),
        fact(FACT, "sixel", yes_no(app.caps.sixel).to_string()),
        fact(
            FACT,
            "character cell",
            app.gfx.as_ref().map_or_else(
                || "not reported".to_string(),
                |gfx| format!("{}x{} px", gfx.cell.0, gfx.cell.1),
            ),
        ),
        fact(
            DECISION,
            "drawing with",
            match app.pixels_available() {
                true => format!("{cells} cells ({})", app.protocol_name()),
                false => format!("{cells} cells — no pixels here"),
            },
        ),
    ];
    if !app.pixels_available() && app.caps.answered {
        lines.push(blank());
        lines.push((
            HINT,
            Line::styled(
                "  kitty, Ghostty, WezTerm, foot, Konsole, iTerm2 and",
                muted,
            ),
        ));
        lines.push((
            HINT,
            Line::styled("  xterm -ti vt340 draw one protocol or the other.", muted),
        ));
    }
    lines.push(blank());
    lines.push((WAY_OUT, Line::styled("  any key closes this", muted)));

    // Drop the cheapest lines until it fits. The panel is a border plus its
    // contents, so three rows of chrome.
    while lines.len() as u16 + 3 > inner.height && lines.len() > 1 {
        let Some(cheapest) = lines
            .iter()
            .enumerate()
            .min_by_key(|(index, (rank, _))| (*rank, std::cmp::Reverse(*index)))
            .map(|(index, _)| index)
        else {
            break;
        };
        lines.remove(cheapest);
    }
    let lines: Vec<Line<'static>> = lines.into_iter().map(|(_, line)| line).collect();

    // Centred, and never larger than the frame it floats over.
    // Wide enough for the longest line at a 16-column key gutter, and never
    // wider than what it floats over.
    let width = 58.min(inner.width);
    let height = (lines.len() as u16 + 2).min(inner.height);
    let area = Rect::new(
        inner.x + (inner.width.saturating_sub(width)) / 2,
        inner.y + (inner.height.saturating_sub(height)) / 2,
        width,
        height,
    );
    let block = Block::bordered()
        .border_type(ratatui::widgets::BorderType::Rounded)
        .border_style(Style::new().fg(palette.ansi(palette.rule)))
        .style(Style::new().bg(palette.ansi(palette.canvas)))
        .title(Line::styled(" help ", key));
    frame.render_widget(Clear, area);
    frame.render_widget(Paragraph::new(lines).block(block), area);
}

fn body(app: &App, inner: Rect) -> (Vec<Line<'static>>, Option<Layout>) {
    let mut lines = vec![header(app), Line::raw("")];
    let mut layout = None;
    match &app.load {
        Load::Loading => lines.push(loading(app)),
        Load::Failed(message) => lines.extend(failure(app, message)),
        Load::Ready(calendar) => {
            // The chart starts two lines down, which is what turns a line index into
            // a screen row.
            let origin = (inner.x, inner.y + lines.len() as u16);
            let (chart, resolved) = chart(app, calendar, inner, origin);
            lines.extend(chart);
            layout = resolved;
        }
    }
    lines.push(Line::raw(""));
    lines.push(footer(app));
    (lines, layout)
}

fn header(app: &App) -> Line<'static> {
    let palette = &app.palette;
    let mut spans = vec![
        Span::styled(
            app.login.clone(),
            Style::new().fg(palette.ansi(palette.fg)).bold(),
        ),
        separator(palette),
        Span::styled(
            app.year.to_string(),
            Style::new().fg(palette.ansi(palette.fg)),
        ),
    ];
    if let Load::Ready(calendar) = &app.load {
        spans.push(separator(palette));
        // github.com's own wording, under its own chart.
        spans.push(Span::styled(
            format!(
                "{} contributions in {}",
                thousands(calendar.total),
                app.year
            ),
            Style::new().fg(palette.ansi(palette.fg)),
        ));
    }
    Line::from(spans)
}

fn separator(palette: &Palette) -> Span<'static> {
    Span::styled("  ·  ", Style::new().fg(palette.ansi(palette.muted)))
}

fn chart(
    app: &App,
    calendar: &Calendar,
    inner: Rect,
    origin: (u16, u16),
) -> (Vec<Line<'static>>, Option<Layout>) {
    if calendar.weeks.is_empty() {
        return (empty(app, calendar), None);
    }

    let weeks = calendar.weeks.len();
    let (width, height) = (inner.width as usize, inner.height as usize);
    let cells = resolve(app.cells, weeks, width, height, app.pixels_available());

    let mut lines = Vec::with_capacity(cells.height() + 5);
    lines.push(months(app, calendar, &cells));
    let grid_row = origin.1 + lines.len() as u16;
    match cells {
        // Nothing but the gutter: the seven rows below belong to the painter, and
        // staying blank is what keeps ratatui's diff from writing over the image.
        Cells::Pixels => lines.extend((0..7).map(|row| Line::from(weekday(app, row)))),
        Cells::Rounded { gap } => lines.extend(filled_rows(app, calendar, ROUND, gap)),
        Cells::Squares => lines.extend(filled_rows(app, calendar, SQUARE, 1)),
        Cells::Grid { fill } => lines.extend(grid_rows(app, calendar, fill)),
        Cells::Solid { fill, gap } => lines.extend(solid_rows(app, calendar, fill, gap)),
    }
    lines.push(Line::raw(""));
    lines.push(detail(app, calendar));

    let legend_row = origin.1 + lines.len() as u16;
    let (legend, legend_at) = legend(app, calendar, &cells, inner.x, legend_row);
    lines.push(legend);
    lines.push(summary(app, calendar));
    if let Some(note) = note(app, &cells, weeks, width, height) {
        lines.push(note);
    }

    let layout = Layout {
        x: origin.0 + (GUTTER + cells.offset()) as u16,
        y: grid_row,
        weeks: weeks as u16,
        cells,
        legend: legend_at,
        bottom: inner.bottom(),
        right: inner.right(),
    };
    (lines, Some(layout))
}

/// The line under the chart that explains why it does not look its best.
fn note(
    app: &App,
    cells: &Cells,
    weeks: usize,
    width: usize,
    height: usize,
) -> Option<Line<'static>> {
    let muted = Style::new().fg(app.palette.ansi(app.palette.muted));
    // Asking for a protocol and getting characters is the one outcome
    // `--capabilities` explained and the chart did not.
    if let Some(protocol) = app.graphics_refused() {
        return Some(Line::styled(
            format!("({protocol} was asked for, but no cell size is known — pass --cell WxH)"),
            muted,
        ));
    }
    if !cells.fits(weeks, width, height) {
        // Telling someone to press `d` for something smaller only helps while
        // something smaller exists. From `Auto` at its narrowest, `d` goes to
        // pixels — three times wider — so the advice made it worse.
        let smallest = Cells::Solid { fill: 1, gap: 0 };
        let short = smallest.height() + CHROME_ROWS > height;
        let narrow = smallest.width(weeks) > width;
        return Some(Line::styled(
            match (narrow, short) {
                (true, _) => format!(
                    "(a year needs {} columns even at its narrowest — this window has {})",
                    smallest.width(weeks) + BORDER,
                    width + BORDER
                ),
                (false, true) => format!(
                    "(the chart needs {} rows — this window has {})",
                    smallest.height() + CHROME_ROWS + BORDER,
                    height + BORDER
                ),
                _ => "(too small for these cells — press d for a smaller style)".to_string(),
            },
            muted,
        ));
    }
    if !matches!(app.cells, CellStyle::Auto) {
        return None;
    }
    // Every style Auto can choose is seven rows tall, so width is the only thing
    // that rules the better ones out — if height were short, nothing would fit and
    // the branch above would have fired. Say what it wants, because silently
    // sharpening the corners reads as rounding not working.
    let (wanted, need) = match (app.pixels_available(), cells) {
        (true, Cells::Pixels) => return None,
        (true, _) => ("pixel cells", Cells::Pixels.width(weeks) + BORDER),
        (false, Cells::Rounded { .. }) => return None,
        (false, _) => (
            "rounded corners",
            Cells::Rounded { gap: 1 }.width(weeks) + BORDER,
        ),
    };
    Some(Line::styled(
        format!("{wanted} need {need} columns — d forces them, clipped"),
        muted,
    ))
}

/// Nothing to draw at all. Only reachable if GitHub returns an empty range, since
/// a real year always yields its days, future ones included.
fn empty(app: &App, calendar: &Calendar) -> Vec<Line<'static>> {
    let year = calendar.year;
    let muted = Style::new().fg(app.palette.ansi(app.palette.muted));
    let reason = if calendar.starts_after(Local::now().date_naive()) {
        format!("{year} hasn't started yet — nothing to draw until Jan 1 {year}")
    } else {
        format!("no contribution data for {year}")
    };
    vec![
        Line::styled(reason, muted),
        Line::raw(""),
        Line::styled("[ previous year  ·  u change user", muted),
    ]
}

fn months(app: &App, calendar: &Calendar, cells: &Cells) -> Line<'static> {
    let start = GUTTER + cells.offset();
    let mut label = " ".repeat(start);
    for (week, name) in calendar.month_labels() {
        let column = start + week * cells.stride();
        // Skip a label that the previous one has already run into.
        if column < label.len() {
            continue;
        }
        label.push_str(&" ".repeat(column - label.len()));
        label.push_str(name);
    }
    Line::styled(label, Style::new().fg(app.palette.ansi(app.palette.fg)))
}

/// Box-bordered cells. Neighbours share a border, so each week costs two columns
/// and each weekday two rows.
fn grid_rows(app: &App, calendar: &Calendar, fill: usize) -> Vec<Line<'static>> {
    let weeks = calendar.weeks.len();
    let border = Style::new().fg(app.palette.ansi(app.palette.rule));
    let gutter = || Span::raw(" ".repeat(GUTTER));

    let rule = |left: char, joint: char, right: char| {
        let mut drawn = String::with_capacity(weeks * (fill + 1) + 2);
        drawn.push(left);
        for week in 0..weeks {
            if week > 0 {
                drawn.push(joint);
            }
            for _ in 0..fill {
                drawn.push('─');
            }
        }
        drawn.push(right);
        Line::from(vec![gutter(), Span::styled(drawn, border)])
    };

    let mut lines = Vec::with_capacity(Cells::Grid { fill }.height());
    lines.push(rule('╭', '┬', '╮'));
    for row in 0..7 {
        if row > 0 {
            lines.push(rule('├', '┼', '┤'));
        }
        let mut spans = Vec::with_capacity(weeks * 2 + 2);
        spans.push(weekday(app, row));
        spans.push(Span::styled("│", border));
        for week in &calendar.weeks {
            spans.push(cell(app, week.days[row], fill));
            spans.push(Span::styled("│", border));
        }
        lines.push(Line::from(spans));
    }
    lines.push(rule('╰', '┴', '╯'));
    lines
}

/// github.com's shape: a filled cell per day with a gap column and no outline at
/// all. `glyph` decides the corners — [`SQUARE`] leaves them sharp, [`ROUND`] shaves
/// them. Neither paints a background, so the space around a cell stays the gap.
fn filled_rows(
    app: &App,
    calendar: &Calendar,
    glyph: &'static str,
    gap: usize,
) -> Vec<Line<'static>> {
    // The gap counts: a skipped day has to be as wide as a drawn one plus its gap,
    // otherwise the partial first week shifts the rows above it out of column.
    let blank = " ".repeat(glyph.chars().count() + gap);
    (0..7)
        .map(|row| {
            let mut spans = Vec::with_capacity(calendar.weeks.len() * 2 + 1);
            spans.push(weekday(app, row));
            for week in &calendar.weeks {
                match week.days[row] {
                    // Outside the year: nothing to draw.
                    None => spans.push(Span::raw(blank.clone())),
                    // Not yet happened. Nothing is drawn *unless* it is marked —
                    // the cursor can be walked into the future, `detail` names the
                    // day, and it used to vanish here while the bordered styles
                    // showed it.
                    Some(day) if day.future => match mark(app, day.date) {
                        Some(color) => {
                            spans.push(Span::styled(glyph, Style::new().fg(color)));
                            if gap > 0 {
                                spans.push(Span::raw(" ".repeat(gap)));
                            }
                        }
                        None => spans.push(Span::raw(blank.clone())),
                    },
                    Some(day) => {
                        // A background would fill the gap, so a marked day shows as a
                        // colour rather than the shaded-over cell the grid uses.
                        let color = match mark(app, day.date) {
                            Some(color) => color,
                            None => app.palette.level(day.level),
                        };
                        spans.push(Span::styled(glyph, Style::new().fg(color)));
                        if gap > 0 {
                            spans.push(Span::raw(" ".repeat(gap)));
                        }
                    }
                }
            }
            Line::from(spans)
        })
        .collect()
}

fn solid_rows(app: &App, calendar: &Calendar, fill: usize, gap: usize) -> Vec<Line<'static>> {
    (0..7)
        .map(|row| {
            let mut spans = Vec::with_capacity(calendar.weeks.len() * 2 + 1);
            spans.push(weekday(app, row));
            for week in &calendar.weeks {
                spans.push(cell(app, week.days[row], fill));
                if gap > 0 {
                    spans.push(Span::raw(" ".repeat(gap)));
                }
            }
            Line::from(spans)
        })
        .collect()
}

fn weekday(app: &App, row: usize) -> Span<'static> {
    Span::styled(
        format!("{:<width$}", WEEKDAYS[row], width = GUTTER),
        Style::new().fg(app.palette.ansi(app.palette.fg)),
    )
}

/// The colour a day is marked with, if it is: the mouse wins over the cursor, the
/// same way a browser shows the tooltip for what you are pointing at.
fn mark(app: &App, date: NaiveDate) -> Option<ratatui::style::Color> {
    if app.hover == Some(date) {
        return Some(app.palette.ansi(app.palette.accent));
    }
    (app.cursor == date).then(|| app.palette.ansi(app.palette.fg))
}

/// One day. Days outside the year and days still to come both draw blank, so the
/// year is always a complete rectangle with the unwritten part left empty.
fn cell(app: &App, day: Option<Day>, fill: usize) -> Span<'static> {
    let Some(day) = day else {
        return Span::raw(" ".repeat(fill));
    };
    let marked = mark(app, day.date);
    if day.future {
        return match marked {
            Some(color) => Span::styled(CURSOR.repeat(fill), Style::new().fg(color)),
            None => Span::raw(" ".repeat(fill)),
        };
    }
    match marked {
        Some(color) => Span::styled(
            CURSOR.repeat(fill),
            Style::new().fg(color).bg(app.palette.level(day.level)),
        ),
        None => Span::styled(
            FILL.repeat(fill),
            Style::new().fg(app.palette.level(day.level)),
        ),
    }
}

fn detail(app: &App, calendar: &Calendar) -> Line<'static> {
    let palette = &app.palette;
    let Some(day) = calendar.day(app.cursor) else {
        return Line::raw("");
    };
    let date = Span::styled(
        app.cursor.format("%a, %b %-d %Y").to_string(),
        Style::new().fg(palette.ansi(palette.fg)).bold(),
    );
    if day.future {
        return Line::from(vec![
            date,
            separator(palette),
            Span::styled(
                "still to come",
                Style::new().fg(palette.ansi(palette.muted)),
            ),
        ]);
    }
    Line::from(vec![
        date,
        separator(palette),
        Span::styled(count(day.count), Style::new().fg(palette.ansi(palette.fg))),
    ])
}

/// Returns the line and, in pixel mode, where the swatch image goes.
fn legend(
    app: &App,
    calendar: &Calendar,
    cells: &Cells,
    x: u16,
    row: u16,
) -> (Line<'static>, Option<(u16, u16)>) {
    let palette = &app.palette;
    let muted = Style::new().fg(palette.ansi(palette.muted));
    let lead = "Less ";
    let mut spans = vec![Span::styled(lead, muted)];
    for level in 0..5u8 {
        spans.push(Span::styled(
            cells.swatch(),
            Style::new().fg(palette.level(level)),
        ));
        if cells.swatch_gap() > 0 {
            spans.push(Span::raw(" ".repeat(cells.swatch_gap())));
        }
    }
    // The gap after the last swatch is the one before "More"; pixel swatches carry
    // theirs inside the image, so that one has to be spelled out.
    spans.push(Span::styled(
        if cells.swatch_gap() > 0 {
            "More"
        } else {
            " More"
        },
        muted,
    ));
    if calendar.days().any(|day| day.future) {
        spans.push(Span::styled("   ·   blank = still to come", muted));
    }
    // Naming the protocol matters here: "pixel cells (sixel)" is the difference
    // between a chart that looks right and knowing why it looks right.
    // Naming `auto` matters: pressing `d` from the narrowest style lands back on
    // it, and if the line read the same as the style auto happens to resolve to,
    // the keypress looked like it had done nothing.
    let chosen = match matches!(app.cells, CellStyle::Auto) {
        true => format!("auto: {}", cells.name()),
        false => cells.name().to_string(),
    };
    spans.push(Span::styled(
        match cells {
            Cells::Pixels => format!("   ·   {chosen} cells ({})", app.protocol_name()),
            _ => format!("   ·   {chosen} cells"),
        },
        muted,
    ));
    let at = matches!(cells, Cells::Pixels).then(|| (x + lead.len() as u16, row));
    (Line::from(spans), at)
}

fn summary(app: &App, calendar: &Calendar) -> Line<'static> {
    let muted = Style::new().fg(app.palette.ansi(app.palette.muted));
    if !calendar.has_elapsed_days() {
        return Line::styled(format!("{} hasn't started yet", calendar.year), muted);
    }
    let stats = calendar.stats(app.today());
    let mut parts = vec![format!("{} active days", thousands(stats.active_days))];
    if stats.current_streak > 0 {
        parts.push(format!("{}-day streak", stats.current_streak));
    }
    if stats.longest_streak > 0 {
        parts.push(format!("longest {}", stats.longest_streak));
    }
    if let Some((date, count)) = stats.best {
        parts.push(format!(
            "best {} ({})",
            date.format("%b %-d"),
            thousands(count)
        ));
    }
    Line::styled(parts.join("  ·  "), muted)
}

/// The floating box github.com shows above the day under the pointer, in the same
/// `--bgColor-emphasis` and with the same wording. It sits in the two rows above the
/// grid rather than over it: in pixel mode those rows belong to the image, and text
/// written over a sixel erases it for good.
fn tooltip(frame: &mut Frame<'_>, app: &App, layout: &Layout, inner: Rect) {
    let Load::Ready(calendar) = &app.load else {
        return;
    };
    let (Some(date), Some(day)) = (app.hover, app.hover.and_then(|date| calendar.day(date))) else {
        return;
    };
    let (Some((week, _)), Some(text_row)) = (calendar.position(date), layout.y.checked_sub(2))
    else {
        return;
    };
    if day.future {
        // github.com does not draw a day that has not happened, so it has nothing to
        // say about one either. Reachable only if the pointer is left on a cell that
        // a reload turns into the future.
        return;
    }

    let palette = &app.palette;
    let text = format!(
        "{} on {} {}.",
        count(day.count),
        date.format("%B"),
        ordinal(date.day())
    );
    let width = text.chars().count() as u16 + 4;
    if width > inner.width {
        return;
    }
    // Point at the middle of the day's cell, and keep the box on screen.
    let point =
        layout.x + week as u16 * layout.cells.stride() as u16 + layout.cells.stride() as u16 / 2;
    let x = point
        .saturating_sub(width / 2)
        .clamp(inner.x, inner.right().saturating_sub(width));

    let background = palette.ansi(palette.tooltip_bg);
    // Half blocks close the ends, so the box reads as a rounded pill rather than as
    // a rectangle of colour bleeding into the cell beside it.
    let pill = Line::from(vec![
        Span::styled("▐", Style::new().fg(background)),
        Span::styled(
            format!(" {text} "),
            Style::new()
                .bg(background)
                .fg(palette.ansi(palette.tooltip_fg)),
        ),
        Span::styled("▌", Style::new().fg(background)),
    ]);
    frame.render_widget(Paragraph::new(pill), Rect::new(x, text_row, width, 1));
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled("▼", Style::new().fg(background)))),
        Rect::new(
            point.min(inner.right().saturating_sub(1)),
            text_row + 1,
            1,
            1,
        ),
    );
}

/// "No contributions" / "1 contribution" / "97 contributions", as github.com writes it.
fn count(count: u32) -> String {
    match count {
        0 => "No contributions".to_string(),
        1 => "1 contribution".to_string(),
        n => format!("{} contributions", thousands(n)),
    }
}

fn ordinal(day: u32) -> String {
    let suffix = match (day % 10, day % 100) {
        (_, 11..=13) => "th",
        (1, _) => "st",
        (2, _) => "nd",
        (3, _) => "rd",
        _ => "th",
    };
    format!("{day}{suffix}")
}

fn loading(app: &App) -> Line<'static> {
    let frame = SPINNER[(app.tick / 2) as usize % SPINNER.len()];
    Line::from(vec![
        Span::styled(frame, Style::new().fg(app.palette.level(4))),
        Span::styled(
            format!(" loading {} {}…", app.login, app.year),
            Style::new().fg(app.palette.ansi(app.palette.muted)),
        ),
    ])
}

fn failure(app: &App, message: &str) -> Vec<Line<'static>> {
    let palette = &app.palette;
    vec![
        Line::from(vec![
            Span::styled("!", Style::new().fg(ratatui::style::Color::Red).bold()),
            Span::styled(
                format!(" {message}"),
                Style::new().fg(palette.ansi(palette.fg)),
            ),
        ]),
        Line::raw(""),
        Line::styled(
            "r retry  ·  u change user",
            Style::new().fg(palette.ansi(palette.muted)),
        ),
    ]
}

fn footer(app: &App) -> Line<'static> {
    let palette = &app.palette;
    let muted = Style::new().fg(palette.ansi(palette.muted));
    match &app.mode {
        Mode::Input(buffer) => Line::from(vec![
            Span::styled("user ", muted),
            Span::styled(
                buffer.clone(),
                Style::new().fg(palette.ansi(palette.fg)).bold(),
            ),
            Span::styled("▏", Style::new().fg(palette.ansi(palette.fg))),
            Span::styled("   enter load  ·  esc cancel", muted),
        ]),
        Mode::Normal => {
            // Assembled longest-first and then trimmed, because the one thing
            // that must survive a narrow terminal is how to get out of it. At 80
            // columns the joined line was cut mid-word and took `q quit` and
            // `? help` with it, while the overlay that would have said so was
            // itself truncated.
            let mut keys = vec!["←→↑↓ day/week"];
            if !app.previewing() {
                keys.push("[ ] year");
            }
            keys.push("t today");
            if !app.previewing() {
                keys.push("u user");
            }
            keys.push("d cells");
            keys.push(if app.mouse {
                "m mouse off"
            } else {
                "m mouse on"
            });
            keys.push("r reload");
            keys.push("q quit");
            keys.push("? help");
            if let Some(label) = app.source_label() {
                keys.push(label);
            }

            const JOIN: &str = "  ·  ";
            // Columns, not bytes: the separator's `·` is two bytes wide and one
            // column, and it is the width that has to fit.
            const JOIN_COLUMNS: usize = 5;
            let width = usize::from(app.footer_width);
            let fits = |items: &[&str]| {
                items.iter().map(|item| item.chars().count()).sum::<usize>()
                    + JOIN_COLUMNS * items.len().saturating_sub(1)
                    <= width
            };
            // Dropped from the front, where the movement hints are — those are the
            // guessable ones — and never past the last two, which are `q quit`
            // and `? help`.
            while !fits(&keys) && keys.len() > 2 {
                keys.remove(0);
            }
            Line::styled(keys.join(JOIN), muted)
        }
    }
}
