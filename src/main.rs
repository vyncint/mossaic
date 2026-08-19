//! mossaic — a GitHub contribution chart for the terminal: argument parsing,
//! terminal setup and the event loop. Everything it draws with lives in the
//! library beside it.

use std::io;
use std::time::Duration;

use chrono::{Datelike, Local};
use ratatui::backend::Backend;
use ratatui::crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyEventKind,
};
use ratatui::crossterm::execute;
use ratatui::crossterm::terminal::{BeginSynchronizedUpdate, EndSynchronizedUpdate};
use ratatui::DefaultTerminal;

use mossaic::app::{App, Graphics, Options, Source};
use mossaic::cli::Args;
use mossaic::primer::{Appearance, Palette, Season};
use mossaic::{github, graphics, png, term, ui};

const HELP: &str = "\
mossaic — GitHub's contribution chart in your terminal

usage:
  mossaic [username] [options]

examples:
  mossaic                     your chart, this year
  mossaic --demo              a sample year — no account, no network
  mossaic octocat -y 2024     someone else, some year
  mossaic --capabilities      what your terminal can draw
  mossaic --png chart.png     the chart as an image

options:
  -y, --year YEAR   calendar year to show (default: this year)
      --demo        a sample year, for trying it without `gh`
  -f, --file PATH   render a saved calendar instead of calling gh
                    (the `mossaic-art` binary writes them)
      --png PATH    write the chart to a PNG and exit — the same pixels the
                    graphics protocols would draw, from any terminal
  -g, --graphics M  auto | kitty | sixel | text  (default: ask the terminal)
      --cell WxH    size of one character cell in pixels, when the terminal
                    will not say (e.g. --cell 10x20)
      --theme M     auto | dark | light | dimmed (default: ask the terminal)
      --palette P   auto | default | winter | halloween — GitHub's own seasonal
                    colours; auto follows the calendar, as github.com does
      --no-mouse    do not turn on mouse reporting
      --capabilities  ask the terminal what it can do, print it, and exit
  -V, --version     print the version
  -h, --help        show this help

in the chart:
  ?                 keys, mouse, and what this terminal can draw
  arrows / h j k l  move a day or a week
  [ ]               previous / next year
  hover a day       its tooltip; click moves the cursor; wheel changes year
  d                 cycle cell style        m  mouse reporting off/on
  t  today          r  reload               q  quit

also installed:
  mossaic-art      write text into a contribution graph, and track the plan
  mossaic-glyphs   what this terminal makes of the fallback cells

Needs the GitHub CLI (https://cli.github.com) for a real account; run
`gh auth login` once. `--demo`, `--file` and `--png` need nothing.";

const VERSION: &str = concat!("mossaic ", env!("CARGO_PKG_VERSION"));

/// Redraw cadence. Also bounds how long a finished fetch waits to appear.
const TICK: Duration = Duration::from_millis(80);
/// How long to wait for a terminal to say what it can do. A terminal that answers
/// takes a millisecond or two; this is the budget for one that never will.
const PROBE: Duration = Duration::from_millis(250);

/// Everything the command line settles before a terminal is involved.
struct Invocation {
    login: String,
    year: i32,
    source: Source,
    options: Options,
    /// Render to a file and exit, instead of running the chart.
    png: Option<String>,
}

fn main() {
    let Some(invocation) = parse_args() else {
        return;
    };
    if let Some(path) = &invocation.png {
        write_png(&invocation, path);
        return;
    }
    let Invocation {
        login,
        year,
        source,
        options,
        ..
    } = invocation;

    // try_init rather than init/run so a non-tty (a pipe, CI) gets a readable
    // message instead of a panic. It still installs the terminal-restoring hook.
    let mut terminal = ratatui::try_init()
        .unwrap_or_else(|error| fail(&format!("needs an interactive terminal ({error})")));

    // Ask now: raw mode is on, so the replies come back unbuffered and unechoed,
    // and anything a terminal prints instead of answering lands on the alternate
    // screen where the first frame paints over it.
    let mut app = App::new(login, year, source);
    app.configure(term::probe(PROBE), options);
    restore_mouse_on_panic();

    let outcome = run(&mut terminal, &mut app);
    let restored = ratatui::try_restore();

    // Report the run error if there is one, otherwise a failure to restore the terminal.
    if let Some(error) = [outcome.err(), restored.err()].into_iter().flatten().next() {
        eprintln!("mossaic: {error}");
        std::process::exit(1);
    }
}

fn run(terminal: &mut DefaultTerminal, app: &mut App) -> io::Result<()> {
    let mut out = io::stdout();
    let mut mouse = false;
    app.request();

    while !app.quit {
        if app.mouse != mouse {
            mouse = app.mouse;
            if mouse {
                execute!(out, EnableMouseCapture)?;
            } else {
                execute!(out, DisableMouseCapture)?;
            }
        }
        app.drain();
        if std::mem::take(&mut app.redraw) {
            // Pixels the text layer never wrote are pixels it cannot erase, so
            // anything that moves or removes the image clears the screen first.
            //
            // Not `Terminal::clear`: that reads the cursor position back from the
            // terminal first, and a terminal slow to answer — or one that never
            // does — turns a keystroke into a two-second stall or an error. The
            // screen is cleared directly instead, and swapping the buffers leaves
            // ratatui with nothing to diff against, so the next frame redraws whole.
            terminal.backend_mut().clear()?;
            terminal.swap_buffers();
            if let Some(painter) = &mut app.gfx {
                painter.invalidate();
            }
        }
        // One frame, bracketed: the text goes out through ratatui and the images
        // straight after it, and a terminal that understands DEC 2026 shows the
        // two together instead of a chart that arrives without its cells.
        execute!(out, BeginSynchronizedUpdate)?;
        terminal.draw(|frame| ui::draw(frame, app))?;
        app.paint(&mut out)?;
        execute!(out, EndSynchronizedUpdate)?;

        if event::poll(TICK)? {
            // Drain the queue rather than taking one event per frame: motion
            // reports arrive in floods, and answering them one frame at a time
            // leaves the tooltip trailing several cells behind the pointer.
            loop {
                match event::read()? {
                    Event::Key(key) if key.kind == KeyEventKind::Press => {
                        app.on_key(key.code, key.modifiers);
                    }
                    Event::Mouse(event) => app.on_mouse(event),
                    Event::Resize(..) => app.redraw = true,
                    _ => {}
                }
                if app.quit || !event::poll(Duration::ZERO)? {
                    break;
                }
            }
        }
        app.tick += 1;
    }

    app.stop(&mut out)?;
    if mouse {
        execute!(out, DisableMouseCapture)?;
    }
    Ok(())
}

/// What this terminal answered, and what mossaic will do about it. The whole
/// pixel path turns on replies that are easy to get wrong and impossible to see, so
/// there is one command that shows them.
fn report_capabilities(options: Options) {
    // Raw mode, or the replies come back echoed and line-buffered — the same reason
    // the probe runs after the terminal is set up.
    let raw = ratatui::crossterm::terminal::enable_raw_mode();
    let caps = term::probe(PROBE);
    let cell = term::cell_size(&caps);
    if raw.is_ok() {
        let _ = ratatui::crossterm::terminal::disable_raw_mode();
    }

    let yes_no = |flag: bool| if flag { "yes" } else { "no" };
    println!(
        "terminal   TERM={}",
        std::env::var("TERM").unwrap_or_default()
    );
    if !caps.answered {
        println!("           answered nothing — a pipe, a multiplexer, or too old to ask");
    }
    println!("kitty      {}", yes_no(caps.kitty));
    println!("sixel      {}", yes_no(caps.sixel));
    println!(
        "cell       {}",
        cell.map_or_else(
            || "unknown — without it an image cannot be lined up, so text it is".to_string(),
            |(w, h)| format!("{w}x{h} px"),
        )
    );
    println!(
        "background {}",
        caps.background.map_or_else(
            || "unknown — assuming a dark terminal".to_string(),
            |bg| format!(
                "#{:02x}{:02x}{:02x}  ({:?} theme)",
                bg.0,
                bg.1,
                bg.2,
                Appearance::from_background(bg)
            ),
        )
    );

    let mut app = App::new("preview".to_string(), Local::now().year(), Source::GitHub);
    app.configure(caps, options);
    println!("cells      {}", app.protocol_name());
}

/// A panic that unwinds past the event loop would otherwise leave mouse reporting
/// on, and the shell printing escape codes at every click.
fn restore_mouse_on_panic() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = execute!(io::stdout(), DisableMouseCapture);
        previous(info);
    }));
}

/// Returns `None` when there is nothing left to run, e.g. after printing help.
fn parse_args() -> Option<Invocation> {
    let mut login = None;
    let mut year = None;
    let mut file: Option<String> = None;
    let mut png: Option<String> = None;
    let mut demo = false;
    let mut capabilities = false;
    let mut options = Options::default();

    // `--key=value` and `--key value` are the same thing from here on. Only long
    // options split, so a username or path containing `=` is left alone.
    let mut args = Args::from_env("mossaic");

    while let Some(arg) = args.next_arg() {
        match arg.as_str() {
            "-h" | "--help" => {
                println!("{HELP}");
                return None;
            }
            "-V" | "--version" => {
                println!("{VERSION}");
                return None;
            }
            "--demo" => demo = true,
            "-y" | "--year" => year = Some(args.year("--year")),
            "-f" | "--file" => file = Some(args.value("--file")),
            "-g" | "--graphics" => {
                options.graphics = match args.value("--graphics").as_str() {
                    "auto" => Graphics::Auto,
                    "kitty" => Graphics::Kitty,
                    "sixel" => Graphics::Sixel,
                    "text" | "none" | "off" => Graphics::Text,
                    other => fail(&format!(
                        "unknown graphics mode {other:?} — auto, kitty, sixel or text"
                    )),
                }
            }
            "--theme" => {
                options.appearance = match args.value("--theme").as_str() {
                    "auto" => None,
                    "dark" => Some(Appearance::Dark),
                    "light" => Some(Appearance::Light),
                    "dimmed" | "dark-dimmed" => Some(Appearance::Dimmed),
                    other => fail(&format!(
                        "unknown theme {other:?} — auto, dark, light or dimmed"
                    )),
                }
            }
            "--palette" => {
                options.season = match args.value("--palette").as_str() {
                    "auto" => None,
                    "default" => Some(Season::Default),
                    "winter" => Some(Season::Winter),
                    "halloween" => Some(Season::Halloween),
                    other => fail(&format!(
                        "unknown palette {other:?} — auto, default, winter or halloween"
                    )),
                }
            }
            "--no-mouse" => options.mouse = false,
            "--cell" => options.cell = Some(parse_cell(&args.value("--cell"))),
            "--png" => png = Some(args.value("--png")),
            "--capabilities" => capabilities = true,
            other if other.starts_with('-') => {
                fail(&format!("unknown option {other:?} — try --help"))
            }
            other => login = Some(other.to_string()),
        }
    }

    // After the loop, so that flags on either side of it are all in hand.
    if capabilities {
        report_capabilities(options);
        return None;
    }

    // The demo makes its own year up, so it needs no account and no network.
    if demo {
        return Some(Invocation {
            login: login.unwrap_or_else(|| "demo".to_string()),
            year: year.unwrap_or_else(|| Local::now().year() - 1),
            source: Source::Demo,
            options,
            png,
        });
    }

    // A file carries its own login and year, so neither needs looking up.
    if let Some(path) = file {
        return Some(Invocation {
            login: login.unwrap_or_else(|| "preview".to_string()),
            year: year.unwrap_or_else(|| Local::now().year()),
            source: Source::File(path),
            options,
            png,
        });
    }

    let login = login.or_else(github::whoami).unwrap_or_else(|| {
        fail(
            "could not detect a GitHub user.\n\n  \
             mossaic <username>   chart someone by name\n  \
             gh auth login          authenticate, then just `mossaic`\n  \
             mossaic --demo       see what it looks like first",
        )
    });
    Some(Invocation {
        login,
        year: year.unwrap_or_else(|| Local::now().year()),
        source: Source::GitHub,
        options,
        png,
    })
}

/// The chart as a file, for a terminal that draws no pixels — and for a README.
/// The rasteriser is the same one the protocols feed from, so this is what they
/// would have drawn, not an approximation of it.
fn write_png(invocation: &Invocation, path: &str) {
    let calendar = match &invocation.source {
        Source::GitHub => github::fetch(&invocation.login, invocation.year),
        Source::File(file) => github::from_file(file),
        Source::Demo => Ok(mossaic::calendar::demo(invocation.year)),
    }
    .unwrap_or_else(|error| fail(&error));

    let palette = Palette::new(
        invocation.options.appearance.unwrap_or(Appearance::Dark),
        invocation
            .options
            .season
            .unwrap_or_else(|| Season::on(Local::now().date_naive())),
        true,
    );
    // No terminal to ask, so a readable default: a cell about twice as tall as
    // it is wide, which is what the chart's proportions assume.
    let cell = invocation.options.cell.unwrap_or((10, 20));
    let levels: Vec<[Option<u8>; 7]> = calendar
        .weeks
        .iter()
        .map(|week| {
            let mut column = [None; 7];
            for (weekday, day) in week.days.iter().enumerate() {
                column[weekday] = day.filter(|day| !day.future).map(|day| day.level);
            }
            column
        })
        .collect();

    let image = graphics::grid(&levels, &palette, cell);
    png::write(std::path::Path::new(path), &image, palette.canvas)
        .unwrap_or_else(|error| fail(&format!("could not write {path}: {error}")));
    println!(
        "wrote {path} — {}x{} px, {} {} at {}x{} per cell",
        image.width, image.height, calendar.login, calendar.year, cell.0, cell.1
    );
}

/// `WIDTHxHEIGHT`, the way a terminal reports a cell.
///
/// Bounded, because this sizes an allocation: `--cell 20000x20000` asks for a
/// terabyte of image.
fn parse_cell(raw: &str) -> (u16, u16) {
    raw.split_once(['x', 'X'])
        .and_then(|(w, h)| Some((w.trim().parse().ok()?, h.trim().parse().ok()?)))
        .filter(|(w, h): &(u16, u16)| {
            (2..=term::MAX_CELL).contains(w) && (2..=term::MAX_CELL).contains(h)
        })
        .unwrap_or_else(|| {
            fail(&format!(
                "--cell wants WIDTHxHEIGHT in pixels, 2 to {} each, not {raw:?}",
                term::MAX_CELL
            ))
        })
}

fn fail(message: &str) -> ! {
    eprintln!("mossaic: {message}");
    std::process::exit(2);
}
