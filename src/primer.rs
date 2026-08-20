//! Primer colours, read out of the stylesheets github.com actually serves rather
//! than transcribed from memory. Every value here is a
//! `--contribution-*` / `--fgColor-*` / `--bgColor-*` custom property lifted from
//! `light-*.css`, `dark-*.css` and `dark_dimmed-*.css`, so the chart is the same
//! green as the one in the browser.

use chrono::{Datelike, NaiveDate};
use ratatui::style::Color;

/// A colour with 8 bits per channel, the form Primer publishes and the form both
/// graphics protocols want.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rgb(pub u8, pub u8, pub u8);

impl Rgb {
    /// From a `0xRRGGBB` literal, which is how Primer writes them.
    pub const fn hex(value: u32) -> Self {
        Self((value >> 16) as u8, (value >> 8) as u8, value as u8)
    }

    /// `self` laid over `under` at `alpha`, straight (non-premultiplied) alpha.
    pub fn over(self, under: Rgb, alpha: f32) -> Rgb {
        let mix = |a: u8, b: u8| {
            (f32::from(a) * alpha + f32::from(b) * (1.0 - alpha))
                .round()
                .clamp(0.0, 255.0) as u8
        };
        Rgb(
            mix(self.0, under.0),
            mix(self.1, under.1),
            mix(self.2, under.2),
        )
    }

    /// Rough brightness, 0..1 — enough to tell a light terminal from a dark one.
    ///
    /// **Not** WCAG relative luminance: the coefficients are applied to
    /// gamma-encoded sRGB with no linearisation, unlike [`Rgb::lab`] below, so mid
    /// grey comes out 0.502 where relative luminance is 0.216. That puts the
    /// light/dark threshold at `#808080` rather than `#bcbcbc`, which is the more
    /// useful place for choosing a terminal theme — but it is not the quantity the
    /// old name claimed.
    pub fn brightness(self) -> f32 {
        let c = |v: u8| f32::from(v) / 255.0;
        0.2126 * c(self.0) + 0.7152 * c(self.1) + 0.0722 * c(self.2)
    }

    /// CIELAB coordinates under a D65 white point.
    ///
    /// RGB distance is not perceptual distance — `#033a16` and `#196c2e` are 22
    /// apart in green alone, and look almost identical. Lab is the space where
    /// "how different do these look" is a straight line, which is the only
    /// question that matters when the art is one shade of green on another.
    pub fn lab(self) -> (f32, f32, f32) {
        // sRGB -> linear.
        let linear = |channel: u8| {
            let c = f32::from(channel) / 255.0;
            if c <= 0.04045 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            }
        };
        let (r, g, b) = (linear(self.0), linear(self.1), linear(self.2));

        // Linear RGB -> CIE XYZ, then normalised against D65.
        let x = (0.412_456_4 * r + 0.357_576_1 * g + 0.180_437_5 * b) / 0.950_47;
        let y = 0.212_672_9 * r + 0.715_152_2 * g + 0.072_175_0 * b;
        let z = (0.019_333_9 * r + 0.119_192 * g + 0.950_304_1 * b) / 1.088_83;

        // The cube root, with the linear segment that keeps it finite near zero.
        let f = |t: f32| {
            if t > 216.0 / 24389.0 {
                t.cbrt()
            } else {
                (841.0 / 108.0) * t + 4.0 / 29.0
            }
        };
        let (fx, fy, fz) = (f(x), f(y), f(z));
        (116.0 * fy - 16.0, 500.0 * (fx - fy), 200.0 * (fy - fz))
    }

    /// How far apart two colours look, as CIE76 ΔE.
    ///
    /// Roughly: under 2 is invisible, 10 is "you would have to be told", and
    /// anything past 35 reads as two different colours at a glance. Measured
    /// across every palette GitHub ships, adjacent contribution levels fall as low
    /// as **9.1** — light + halloween, levels 1 and 2 — and levels two or more
    /// apart never below **35.4**, which is why
    /// [`Shades`](crate::art::Shades) wants a gap of two.
    ///
    /// ```
    /// # use mossaic::primer::Rgb;
    /// let empty = Rgb::hex(0x151b23);   // dark theme, level 0
    /// let full = Rgb::hex(0x56d364);    // dark theme, level 4
    /// assert!(empty.separation(full) > 100.0);
    /// assert_eq!(empty.separation(empty), 0.0);
    /// ```
    pub fn separation(self, other: Rgb) -> f32 {
        let (l1, a1, b1) = self.lab();
        let (l2, a2, b2) = other.lab();
        ((l1 - l2).powi(2) + (a1 - a2).powi(2) + (b1 - b2).powi(2)).sqrt()
    }

    /// As a ratatui colour: 24-bit where the terminal takes it, the nearest of
    /// the 6×6×6 cube or the 24-step grey ramp where it does not.
    pub fn ansi(self, truecolor: bool) -> Color {
        if truecolor {
            Color::Rgb(self.0, self.1, self.2)
        } else {
            Color::Indexed(self.xterm256())
        }
    }

    /// Nearest xterm-256 entry, taking the better of the 6×6×6 cube and the 24-step
    /// grey ramp. Deriving it beats hand-picking indices: every palette below, and
    /// the seasonal ones, get a sensible fallback for free.
    fn xterm256(self) -> u8 {
        const STEPS: [u8; 6] = [0, 95, 135, 175, 215, 255];
        let nearest = |v: u8| {
            STEPS
                .iter()
                .enumerate()
                .min_by_key(|(_, step)| i32::from(**step).abs_diff(i32::from(v)))
                .map_or(0, |(index, _)| index)
        };
        let (r, g, b) = (nearest(self.0), nearest(self.1), nearest(self.2));
        let cube = Rgb(STEPS[r], STEPS[g], STEPS[b]);

        let average = (u32::from(self.0) + u32::from(self.1) + u32::from(self.2)) as f32 / 3.0;
        let step = (((average - 8.0) / 10.0).round()).clamp(0.0, 23.0) as u8;
        let level = 8 + 10 * step;
        let grey = Rgb(level, level, level);

        if distance(self, grey) < distance(self, cube) {
            232 + step
        } else {
            (16 + 36 * r + 6 * g + b) as u8
        }
    }
}

fn distance(a: Rgb, b: Rgb) -> u32 {
    let d = |x: u8, y: u8| {
        let delta = i32::from(x) - i32::from(y);
        (delta * delta) as u32
    };
    d(a.0, b.0) + d(a.1, b.1) + d(a.2, b.2)
}

/// Which of GitHub's themes to colour with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Appearance {
    /// GitHub's light theme.
    Light,
    /// GitHub's dark theme.
    Dark,
    /// GitHub's dark dimmed theme, which `--theme dimmed` asks for.
    Dimmed,
}

impl Appearance {
    /// Pick from the terminal's own background, the way a browser follows the OS.
    pub fn from_background(background: Rgb) -> Self {
        if background.brightness() > 0.5 {
            Self::Light
        } else {
            Self::Dark
        }
    }
}

/// GitHub swaps the greens out for a few days a year. `data-holiday` in its markup,
/// `--contribution-{winter,halloween}-bgColor-*` in its CSS.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Season {
    /// The greens, all year round.
    Default,
    /// Blues.
    Winter,
    /// Oranges and yellows, which github.com shows for the last week of October.
    Halloween,
}

impl Season {
    /// What github.com would be showing on `date`.
    ///
    /// The winter arm was missing, so `--palette auto` could never pick a scale
    /// that is fully implemented, measured for legibility among the nine
    /// [`crate::art::Shades::worst`] reads, and asked for by `--palette winter`.
    ///
    /// GitHub does not publish the windows, so these are the dates it has been
    /// observed using: the week to Christmas for the blue scale, and the last week
    /// of October for the orange one. Treat them as a best reading of github.com
    /// rather than as a specification.
    pub fn on(date: NaiveDate) -> Self {
        match (date.month(), date.day()) {
            (10, 25..=31) => Self::Halloween,
            (12, 19..=25) => Self::Winter,
            _ => Self::Default,
        }
    }
}

/// Every colour the chart draws with, for one appearance and season.
#[derive(Debug, Clone, Copy)]
pub struct Palette {
    /// Which theme these colours came from.
    pub appearance: Appearance,
    /// Which seasonal scale, if any.
    pub season: Season,
    /// Whether the terminal takes 24-bit colour. Only affects text; the graphics
    /// protocols are always truecolor.
    pub truecolor: bool,
    /// `--contribution-default-bgColor-{0..4}`.
    pub levels: [Rgb; 5],
    /// `--contribution-default-borderColor-0`: a colour at 5% over the cell.
    pub edge: Rgb,
    /// `--bgColor-default`, the page behind the chart.
    pub canvas: Rgb,
    /// `--fgColor-default` and `--fgColor-muted`.
    pub fg: Rgb,
    /// `--fgColor-muted`, for the chrome around the chart.
    pub muted: Rgb,
    /// `--borderColor-default`, for the frame.
    pub rule: Rgb,
    /// `--bgColor-emphasis` / `--fgColor-onEmphasis`, which is what a tooltip is.
    pub tooltip_bg: Rgb,
    /// `--fgColor-onEmphasis`, the tooltip's text.
    pub tooltip_fg: Rgb,
    /// `--fgColor-accent`, for the hovered cell's ring.
    pub accent: Rgb,
    /// `--fgColor-danger`, for a day that is lit and should not be.
    pub danger: Rgb,
}

/// 5% — `--contribution-default-borderColor-0` is `#1f23280d` / `#0104090d`.
pub const EDGE_ALPHA: f32 = 0.05;

impl Palette {
    /// The palette for one theme and season.
    pub fn new(appearance: Appearance, season: Season, truecolor: bool) -> Self {
        let mut palette = match appearance {
            Appearance::Light => Self {
                appearance,
                season,
                truecolor,
                levels: [
                    Rgb::hex(0xeff2f5),
                    Rgb::hex(0xaceebb),
                    Rgb::hex(0x4ac26b),
                    Rgb::hex(0x2da44e),
                    Rgb::hex(0x116329),
                ],
                edge: Rgb::hex(0x1f2328),
                canvas: Rgb::hex(0xffffff),
                fg: Rgb::hex(0x1f2328),
                muted: Rgb::hex(0x59636e),
                rule: Rgb::hex(0xd1d9e0),
                tooltip_bg: Rgb::hex(0x25292e),
                tooltip_fg: Rgb::hex(0xffffff),
                accent: Rgb::hex(0x0969da),
                danger: Rgb::hex(0xd1242f),
            },
            Appearance::Dark => Self {
                appearance,
                season,
                truecolor,
                levels: [
                    Rgb::hex(0x151b23),
                    Rgb::hex(0x033a16),
                    Rgb::hex(0x196c2e),
                    Rgb::hex(0x2ea043),
                    Rgb::hex(0x56d364),
                ],
                edge: Rgb::hex(0x010409),
                canvas: Rgb::hex(0x0d1117),
                fg: Rgb::hex(0xf0f6fc),
                muted: Rgb::hex(0x9198a1),
                rule: Rgb::hex(0x3d444d),
                tooltip_bg: Rgb::hex(0x3d444d),
                tooltip_fg: Rgb::hex(0xffffff),
                accent: Rgb::hex(0x4493f8),
                danger: Rgb::hex(0xf85149),
            },
            Appearance::Dimmed => Self {
                appearance,
                season,
                truecolor,
                levels: [
                    Rgb::hex(0x2a313c),
                    Rgb::hex(0x1b4721),
                    Rgb::hex(0x2b6a30),
                    Rgb::hex(0x46954a),
                    Rgb::hex(0x6bc46d),
                ],
                edge: Rgb::hex(0x010409),
                canvas: Rgb::hex(0x212830),
                fg: Rgb::hex(0xd1d7e0),
                muted: Rgb::hex(0x9198a1),
                rule: Rgb::hex(0x3d444d),
                tooltip_bg: Rgb::hex(0x3d444d),
                tooltip_fg: Rgb::hex(0xf0f6fc),
                accent: Rgb::hex(0x478be6),
                danger: Rgb::hex(0xe5534b),
            },
        };

        // A holiday swaps levels 1-4 only: an empty day stays the neutral it was.
        let holiday = match (season, appearance) {
            (Season::Default, _) => None,
            (Season::Winter, Appearance::Light) => Some([0xb6e3ff, 0x54aeff, 0x0969da, 0x0a3069]),
            (Season::Winter, Appearance::Dark) => Some([0x0c2d6b, 0x1158c7, 0x58a6ff, 0xcae8ff]),
            (Season::Winter, Appearance::Dimmed) => Some([0x143d79, 0x255ab2, 0x539bf5, 0xc6e6ff]),
            (Season::Halloween, Appearance::Light) => {
                Some([0xf0db3d, 0xffd642, 0xf68c41, 0x1f2328])
            }
            (Season::Halloween, _) => Some([0xfac68f, 0xc46212, 0x984b10, 0xe3d04f]),
        };
        if let Some(colors) = holiday {
            for (level, hex) in palette.levels[1..].iter_mut().zip(colors) {
                *level = Rgb::hex(hex);
            }
        }
        palette
    }

    /// A colour as this terminal can show it.
    pub fn ansi(&self, color: Rgb) -> Color {
        color.ansi(self.truecolor)
    }

    /// A level's colour, which is the one place converting is not good enough.
    ///
    /// The 256-colour cube has six steps per channel, and derived indices collide
    /// where it matters most: in the dark theme levels **0 and 1** (`#151b23` and
    /// `#033a16`) both land on grey 234, and in the dimmed theme levels 0 and 1
    /// both land on grey 236 — an empty day and a quiet one drawn identically. The
    /// dimmed ramp is otherwise non-decreasing in luminance, so the defect is the
    /// tie rather than an inversion. So the five levels are chosen rather than
    /// derived, because a legible ramp beats an accurate one that cannot be read.
    /// Everything else still converts: no other colour has to stay distinct from
    /// its neighbour.
    pub fn level(&self, level: u8) -> Color {
        let level = usize::from(level).min(4);
        if self.truecolor {
            return self.ansi(self.levels[level]);
        }
        let light = self.appearance == Appearance::Light;
        let ramp: [u8; 5] = match (self.season, light) {
            //         empty  ---------- more ---------->
            (Season::Default, false) => [236, 22, 28, 34, 40],
            (Season::Default, true) => [254, 157, 78, 35, 22],
            (Season::Winter, false) => [236, 17, 26, 75, 195],
            (Season::Winter, true) => [254, 153, 75, 26, 17],
            (Season::Halloween, false) => [236, 223, 166, 130, 185],
            (Season::Halloween, true) => [254, 227, 220, 208, 235],
        };
        Color::Indexed(ramp[level])
    }

    /// A cell as it ends up on screen: the 5% edge is drawn over the fill, so text
    /// mode — which has no room for a half-pixel border — approximates it by
    /// blending, keeping both modes the same colour.
    pub fn cell(&self, level: u8) -> Rgb {
        self.levels[usize::from(level).min(4)]
    }

    /// How far apart two of the five shades look in this palette, as CIE76 ΔE.
    ///
    /// This is what decides whether contribution art drawn as one shade on
    /// another can be read at all. The seasonal palettes are not uniform — the
    /// halloween ramp puts ΔE 17 between levels 2 and 3 and ΔE 85 between 0 and
    /// 1 — so the answer depends on which palette the reader is looking at.
    pub fn separation(&self, a: u8, b: u8) -> f32 {
        self.cell(a).separation(self.cell(b))
    }

    /// Whether the terminal advertises 24-bit colour.
    pub fn truecolor_env() -> bool {
        std::env::var("COLORTERM").is_ok_and(|v| v.contains("truecolor") || v.contains("24bit"))
    }
}

/// How well two shades tell each other apart.
///
/// The bands come from measuring every palette GitHub ships rather than from
/// taste: adjacent contribution levels fall as low as ΔE 9.1 (light + halloween,
/// levels 1 and 2), while levels two or more apart never drop below ΔE 35.4.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Legibility {
    /// Under ΔE 20. The art is there, but a reader has to be told where to look.
    Faint,
    /// ΔE 20 to 35. Readable, though not at a glance on a small graph.
    Readable,
    /// ΔE 35 and up. Two plainly different colours — what a two-level gap buys.
    Clear,
}

impl Legibility {
    /// The band a separation falls in.
    pub fn of(separation: f32) -> Self {
        if separation >= 35.0 {
            Self::Clear
        } else if separation >= 20.0 {
            Self::Readable
        } else {
            Self::Faint
        }
    }

    /// One word for a report.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Faint => "faint",
            Self::Readable => "readable",
            Self::Clear => "clear",
        }
    }
}

impl std::fmt::Display for Legibility {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}
