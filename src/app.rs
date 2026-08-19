//! Application state, key and mouse handling, and the background fetch plumbing.

use std::io::{self, Write};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

use chrono::{Duration, Local, NaiveDate};
use ratatui::crossterm::event::{KeyCode, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};

use crate::calendar::Calendar;
use crate::github;
use crate::graphics::{Mark, Painter, Protocol, Ring, Scene};
use crate::primer::{Appearance, Palette, Season};
use crate::term::{self, Caps};
use crate::ui::{Cells, Layout};

/// Where the calendar comes from.
#[derive(Debug, Clone)]
pub enum Source {
    /// The GitHub API, through `gh`.
    GitHub,
    /// A saved calendar file, for previewing offline.
    File(String),
    /// A sample year, for trying the chart with no account and no network.
    Demo,
}

/// Where the current year's fetch has got to.
#[derive(Debug)]
pub enum Load {
    /// A fetch is in flight.
    Loading,
    /// A year, ready to draw.
    Ready(Box<Calendar>),
    /// The fetch failed, with what to tell the user.
    Failed(String),
}

/// Which graphics protocol to use, when the choice is not left to the terminal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Graphics {
    /// Whatever the terminal answers to.
    #[default]
    Auto,
    /// The kitty graphics protocol, asked for regardless of the answer.
    Kitty,
    /// Sixel, asked for regardless of the answer.
    Sixel,
    /// Characters only, however capable the terminal turns out to be.
    Text,
}

/// The command line, as far as presentation is concerned.
#[derive(Debug, Clone, Copy)]
pub struct Options {
    /// Which graphics protocol to use, if any.
    pub graphics: Graphics,
    /// `None` follows the terminal's own background colour.
    pub appearance: Option<Appearance>,
    /// `None` follows the calendar, as github.com does.
    pub season: Option<Season>,
    /// Whether to turn on mouse reporting at all.
    pub mouse: bool,
    /// One character cell in pixels, when the terminal will not say. Without it
    /// an image cannot be lined up with the labels around it, so a terminal that
    /// answers nothing gets text cells however capable it is.
    pub cell: Option<(u16, u16)>,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            graphics: Graphics::Auto,
            appearance: None,
            season: None,
            mouse: true,
            cell: None,
        }
    }
}

/// How each day cell is drawn. `Auto` picks the most faithful that fits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CellStyle {
    /// The most faithful style that fits.
    Auto,
    /// Real pixels through the terminal's graphics protocol: github.com's geometry
    /// exactly, at whatever resolution a character cell turns out to be.
    Pixels,
    /// Rounded squares, corners shaved at sub-character resolution with block
    /// sextants. The closest match to github.com without pixels.
    Rounded,
    /// Rounded cells with no gap between them: two thirds of the width, but the
    /// cells touch and read as a chain rather than as separate squares.
    Snug,
    /// The same shape with sharp corners, in less width.
    Squares,
    /// A box border around every square cell: easiest to read a single day out
    /// of, and the largest at two columns per day plus shared borders.
    Grid,
    /// Two-column cells with a blank column between weeks.
    Spaced,
    /// Two-column cells, touching.
    Blocks,
    /// Bordered but only one column per day, so cells are tall rectangles. Keeps
    /// the borders when a terminal is too narrow for square ones.
    Slim,
    /// One column per day, no border: fits a year in the least space.
    Compact,
}

impl CellStyle {
    /// The next style in the cycle. Pixels are skipped where the terminal cannot
    /// draw them, since asking for them there resolves to rounded and pressing `d`
    /// would look like it had done nothing.
    fn next(self, pixels: bool) -> Self {
        let next = match self {
            Self::Auto => Self::Pixels,
            Self::Pixels => Self::Rounded,
            Self::Rounded => Self::Snug,
            Self::Snug => Self::Squares,
            Self::Squares => Self::Grid,
            Self::Grid => Self::Spaced,
            Self::Spaced => Self::Blocks,
            Self::Blocks => Self::Slim,
            Self::Slim => Self::Compact,
            Self::Compact => Self::Auto,
        };
        match next {
            Self::Pixels if !pixels => next.next(pixels),
            next => next,
        }
    }
}

/// What the keyboard is doing.
#[derive(Debug)]
pub enum Mode {
    /// Moving around the chart.
    Normal,
    /// Typing a username into the footer.
    Input(String),
}

/// Result of one background fetch. `seq` lets stale replies be dropped when the
/// user flips through years faster than the requests come back.
struct Fetched {
    seq: u64,
    result: Result<Calendar, String>,
}

/// The whole application state: what is being shown, what the terminal can do,
/// and where the last frame put things.
#[derive(Debug)]
pub struct App {
    /// The GitHub login being shown.
    pub login: String,
    /// The year being shown.
    pub year: i32,
    /// The fetch for that year.
    pub load: Load,
    /// The keyboard cursor's day.
    pub cursor: NaiveDate,
    /// The day under the mouse, if the terminal reports motion and the pointer is
    /// over one. Separate from the cursor: a pointer and a caret are different
    /// things, and github.com's tooltip follows the pointer.
    pub hover: Option<NaiveDate>,
    /// Years with contributions, ascending. Empty until the first fetch lands.
    pub years: Vec<i32>,
    /// Normal, or typing a username.
    pub mode: Mode,
    /// The cell style asked for, which `Auto` leaves to the renderer.
    pub cells: CellStyle,
    /// The colours in use.
    pub palette: Palette,
    /// What the terminal answered at startup.
    pub caps: Caps,
    /// Present when the terminal draws pixels and we know how big a cell is.
    pub gfx: Option<Painter>,
    /// Where the last frame put the grid. Set by the renderer, read by the mouse
    /// and the painter.
    pub layout: Option<Layout>,
    /// Whether mouse reporting is on; `m` toggles it.
    pub mouse: bool,
    /// Whether the help overlay is up. `?` opens it, any key closes it.
    pub help: bool,
    /// Ask for a full clear before the next frame — anything that leaves stale
    /// pixels behind sets it.
    pub redraw: bool,
    /// Where calendars come from.
    pub source: Source,
    /// Frames drawn, which drives the loading spinner.
    pub tick: u64,
    /// Set when the event loop should stop.
    pub quit: bool,
    seq: u64,
    tx: Sender<Fetched>,
    rx: Receiver<Fetched>,
}

impl App {
    /// A fresh app, before the terminal has been asked anything.
    pub fn new(login: String, year: i32, source: Source) -> Self {
        let (tx, rx) = mpsc::channel();
        Self {
            login,
            year,
            load: Load::Loading,
            cursor: Local::now().date_naive(),
            hover: None,
            years: Vec::new(),
            mode: Mode::Normal,
            cells: CellStyle::Auto,
            palette: Palette::new(
                Appearance::Dark,
                Season::on(Local::now().date_naive()),
                Palette::truecolor_env(),
            ),
            caps: Caps::default(),
            gfx: None,
            layout: None,
            mouse: false,
            help: false,
            redraw: false,
            source,
            tick: 0,
            quit: false,
            seq: 0,
            tx,
            rx,
        }
    }

    /// Settle everything that depends on the terminal: which theme, which protocol,
    /// how big a character cell is. Called once, after the terminal is set up and
    /// has been asked what it can do.
    pub fn configure(&mut self, caps: Caps, options: Options) {
        self.caps = caps;
        self.mouse = options.mouse;
        let appearance = options
            .appearance
            .or_else(|| caps.background.map(Appearance::from_background))
            .unwrap_or(Appearance::Dark);
        let season = options
            .season
            .unwrap_or_else(|| Season::on(Local::now().date_naive()));
        self.palette = Palette::new(appearance, season, Palette::truecolor_env());
        self.gfx = painter(&caps, &options, &self.palette);
    }

    /// Whether this terminal can draw pixel cells.
    pub fn pixels_available(&self) -> bool {
        self.gfx.is_some()
    }

    /// The protocol in use, or `text` where there is none.
    pub fn protocol_name(&self) -> &'static str {
        self.gfx.as_ref().map_or("text", |gfx| gfx.protocol.name())
    }

    /// Kick off a fetch for the current login and year, superseding any in flight.
    pub fn request(&mut self) {
        self.seq += 1;
        let (seq, tx) = (self.seq, self.tx.clone());
        let (login, year) = (self.login.clone(), self.year);
        let source = self.source.clone();
        self.load = Load::Loading;
        self.hover = None;
        // The chart is about to be replaced by a spinner, and the rows it occupied
        // are blank as far as the text layer is concerned — so nothing would write
        // over the pixels still sitting in them.
        self.redraw = true;
        thread::spawn(move || {
            let result = match source {
                Source::GitHub => github::fetch(&login, year),
                Source::File(path) => github::from_file(&path),
                Source::Demo => Ok(crate::calendar::demo(year)),
            };
            let _ = tx.send(Fetched { seq, result });
        });
    }

    /// Apply any completed fetches. Non-blocking; call once per frame.
    pub fn drain(&mut self) {
        while let Ok(fetched) = self.rx.try_recv() {
            if fetched.seq != self.seq {
                continue;
            }
            match fetched.result {
                Ok(calendar) => {
                    if !calendar.years.is_empty() {
                        self.years = calendar.years.clone();
                    }
                    self.login = calendar.login.clone();
                    self.year = calendar.year;
                    // Start on today when it is in range, otherwise the newest day shown.
                    let today = Local::now().date_naive();
                    self.cursor = if calendar.day(today).is_some() {
                        today
                    } else {
                        calendar.last_date().unwrap_or(self.cursor)
                    };
                    self.load = Load::Ready(Box::new(calendar));
                }
                Err(message) => self.load = Load::Failed(message),
            }
        }
    }

    /// A file or the demo has no other years or users to move between.
    pub fn previewing(&self) -> bool {
        matches!(self.source, Source::File(_) | Source::Demo)
    }

    /// What the footer calls this, when it is not a real account.
    pub fn source_label(&self) -> Option<&'static str> {
        match self.source {
            Source::GitHub => None,
            Source::File(_) => Some("preview"),
            Source::Demo => Some("demo — `mossaic <user>` for a real one"),
        }
    }

    /// Handle a key press.
    pub fn on_key(&mut self, code: KeyCode, mods: KeyModifiers) {
        if mods.contains(KeyModifiers::CONTROL) && matches!(code, KeyCode::Char('c' | 'd')) {
            self.quit = true;
            return;
        }
        if matches!(self.mode, Mode::Input(_)) {
            self.on_key_input(code);
        } else {
            self.on_key_normal(code);
        }
    }

    fn on_key_normal(&mut self, code: KeyCode) {
        // The help is modal and forgiving: whatever you press to get out of it,
        // gets you out of it. Closing needs a redraw because the overlay is text
        // written over the image, and a sixel does not survive that.
        if self.help {
            self.help = false;
            self.redraw = true;
            return;
        }
        match code {
            KeyCode::Char('q') | KeyCode::Esc => self.quit = true,
            KeyCode::Left | KeyCode::Char('h') => self.move_cursor(-7),
            KeyCode::Right | KeyCode::Char('l') => self.move_cursor(7),
            KeyCode::Up | KeyCode::Char('k') => self.move_cursor(-1),
            KeyCode::Down | KeyCode::Char('j') => self.move_cursor(1),
            // `y` and `Y` used to do this too, undocumented. In a chart that
            // binds `h j k l`, `y` is a key a vim-handed user presses meaning
            // *yank* — and having it silently change the year was the one
            // binding here with a real chance of surprising someone. `PageUp`
            // and `PageDown` say what they do and are documented instead.
            KeyCode::Char('[') | KeyCode::PageUp => self.change_year(-1),
            KeyCode::Char(']') | KeyCode::PageDown => self.change_year(1),
            KeyCode::Home => self.jump(Edge::First),
            KeyCode::End => self.jump(Edge::Last),
            KeyCode::Char('t') => self.jump(Edge::Today),
            // A style change moves or removes the image, so the screen has to go
            // with it: pixels the text layer never wrote are pixels it cannot clear.
            KeyCode::Char('d') => {
                self.cells = self.cells.next(self.pixels_available());
                self.redraw = true;
            }
            KeyCode::Char('?') | KeyCode::F(1) => self.help = true,
            KeyCode::Char('m') => {
                self.mouse = !self.mouse;
                self.hover = None;
            }
            KeyCode::Char('r') => self.request(),
            KeyCode::Char('u') if !self.previewing() => {
                self.mode = Mode::Input(self.login.clone());
                self.hover = None;
            }
            _ => {}
        }
    }

    fn on_key_input(&mut self, code: KeyCode) {
        let Mode::Input(buffer) = &mut self.mode else {
            return;
        };
        match code {
            KeyCode::Backspace => {
                buffer.pop();
            }
            KeyCode::Char(c) if !c.is_whitespace() => buffer.push(c),
            KeyCode::Esc => self.mode = Mode::Normal,
            KeyCode::Enter => {
                let login = buffer.trim().to_string();
                self.mode = Mode::Normal;
                if !login.is_empty() && login != self.login {
                    self.login = login;
                    self.years.clear();
                    self.request();
                }
            }
            _ => {}
        }
    }

    /// Motion, drag and press all hover, because not every terminal reports all
    /// three: `1003` motion tracking is the one that makes hovering work, and where
    /// it is missing — Terminal.app, some multiplexers — a click still lands.
    pub fn on_mouse(&mut self, event: MouseEvent) {
        if matches!(self.mode, Mode::Input(_)) {
            return;
        }
        match event.kind {
            MouseEventKind::Moved | MouseEventKind::Drag(MouseButton::Left) => {
                self.hover = self.day_at(event.column, event.row);
            }
            MouseEventKind::Down(MouseButton::Left) => {
                if let Some(date) = self.day_at(event.column, event.row) {
                    self.cursor = date;
                    self.hover = Some(date);
                }
            }
            MouseEventKind::ScrollUp => self.change_year(-1),
            MouseEventKind::ScrollDown => self.change_year(1),
            _ => {}
        }
    }

    /// Which day is under a character cell. Days still to come are not days: GitHub
    /// draws nothing there and has nothing to say about them.
    fn day_at(&self, column: u16, row: u16) -> Option<NaiveDate> {
        let Load::Ready(calendar) = &self.load else {
            return None;
        };
        let (week, weekday) = self.layout?.hit(column, row)?;
        let day = calendar.weeks.get(week)?.days[weekday]?;
        (!day.future).then_some(day.date)
    }

    /// Move the cursor by whole days (+-1) or weeks (+-7), clamped to the visible range.
    /// Every date between the first and last day exists, so clamping is enough.
    fn move_cursor(&mut self, days: i64) {
        let Load::Ready(calendar) = &self.load else {
            return;
        };
        let (Some(first), Some(last)) = (calendar.first_date(), calendar.last_date()) else {
            return;
        };
        self.cursor = (self.cursor + Duration::days(days)).clamp(first, last);
    }

    fn jump(&mut self, edge: Edge) {
        let Load::Ready(calendar) = &self.load else {
            return;
        };
        let target = match edge {
            Edge::First => calendar.first_date(),
            Edge::Last => calendar.last_date(),
            Edge::Today => {
                let today = Local::now().date_naive();
                calendar.day(today).map(|_| today)
            }
        };
        if let Some(date) = target {
            self.cursor = date;
        }
    }

    /// Step through the years GitHub reports contributions for, so empty years are skipped.
    fn change_year(&mut self, delta: i32) {
        if self.previewing() {
            return;
        }
        let target = match self.years.binary_search(&self.year) {
            Ok(index) => {
                let next = index as i32 + delta;
                if next < 0 || next as usize >= self.years.len() {
                    return;
                }
                self.years[next as usize]
            }
            // Still loading, or a year outside the contribution set (--year 2010):
            // fall back to stepping by one so those years stay reachable.
            Err(_) => self.year + delta,
        };
        if target != self.year {
            self.year = target;
            self.request();
        }
    }

    /// Put the pixels on the screen, after the text has been drawn. Nothing here
    /// touches ratatui's buffer: the renderer leaves the grid blank and the painter
    /// writes over it, which is what keeps the diff from erasing the image.
    pub fn paint(&mut self, out: &mut impl Write) -> io::Result<()> {
        // Split borrows: the scene reads the loaded year and the palette, the
        // painter is mutated. Disjoint fields, so both can be held at once.
        let scene = scene(
            &self.load,
            self.layout,
            &self.palette,
            self.cursor,
            self.hover,
        );
        let Some(painter) = self.gfx.as_mut() else {
            return Ok(());
        };
        match scene {
            Some(scene) => painter.paint(out, &scene),
            None => painter.clear(out),
        }
    }

    /// Take the chart down before the terminal goes back to the shell.
    pub fn stop(&mut self, out: &mut impl Write) -> io::Result<()> {
        match self.gfx.as_mut() {
            Some(painter) => painter.clear(out),
            None => Ok(()),
        }
    }
}

fn painter(caps: &Caps, options: &Options, palette: &Palette) -> Option<Painter> {
    let protocol = match options.graphics {
        Graphics::Text => return None,
        Graphics::Kitty => Protocol::Kitty,
        Graphics::Sixel => Protocol::Sixel,
        Graphics::Auto if caps.kitty => Protocol::Kitty,
        Graphics::Auto if caps.sixel => Protocol::Sixel,
        Graphics::Auto => return None,
    };
    // Without the size of a character cell an image cannot be lined up with the
    // labels around it, and a chart half a column out is worse than no chart.
    let cell = options
        .cell
        .or_else(|| term::cell_size(caps))
        .filter(|(w, h)| *w >= 2 && *h >= 2)?;
    Some(Painter::new(
        protocol,
        cell,
        caps.background.unwrap_or(palette.canvas),
    ))
}

/// What the painter should be showing, given what the renderer laid out.
fn scene<'a>(
    load: &'a Load,
    layout: Option<Layout>,
    palette: &'a Palette,
    cursor: NaiveDate,
    hover: Option<NaiveDate>,
) -> Option<Scene<'a>> {
    let layout = layout.filter(|layout| layout.cells == Cells::Pixels && layout.has_room())?;
    let Load::Ready(calendar) = load else {
        return None;
    };

    let levels: Vec<[Option<u8>; 7]> = calendar
        .weeks
        .iter()
        .map(|week| {
            let mut column = [None; 7];
            for (weekday, day) in week.days.iter().enumerate() {
                // A day still to come is drawn as nothing at all, the way an
                // unwritten day is on github.com.
                column[weekday] = day.filter(|day| !day.future).map(|day| day.level);
            }
            column
        })
        .collect();

    let mark = |date: Option<NaiveDate>, ring: Ring| -> Option<Mark> {
        let date = date?;
        let (week, weekday) = calendar.position(date)?;
        Some(Mark {
            week: week as u16,
            weekday: weekday as u16,
            level: calendar
                .day(date)
                .filter(|day| !day.future)
                .map(|day| day.level),
            ring,
        })
    };
    // The pointer wins where both land on the same day: two rings on one cell would
    // draw over each other, and erasing one would take the other with it.
    let hovered = mark(hover, Ring::Hover);
    let cursored = mark(
        Some(cursor).filter(|date| hover != Some(*date)),
        Ring::Cursor,
    );

    Some(Scene {
        key: key(&calendar.login, calendar.year, palette, &levels),
        palette,
        grid: (layout.x, layout.y),
        legend: layout.legend,
        levels,
        marks: [cursored, hovered],
    })
}

/// Identity of the base image: anything that would change a pixel of it. Rings are
/// not in it, because they are painted a cell at a time.
fn key(login: &str, year: i32, palette: &Palette, levels: &[[Option<u8>; 7]]) -> u64 {
    use std::hash::{DefaultHasher, Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    (login, year).hash(&mut hasher);
    (palette.appearance as u8, palette.season as u8).hash(&mut hasher);
    for level in palette.levels {
        (level.0, level.1, level.2).hash(&mut hasher);
    }
    levels.hash(&mut hasher);
    hasher.finish()
}

enum Edge {
    First,
    Last,
    Today,
}
