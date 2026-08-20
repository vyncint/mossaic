//! The chart as pixels, for terminals that draw them.
//!
//! Block sextants get the corners down to thirds of a character. Real pixels get
//! them exact: github.com's cell is 11px on a 14px pitch with a 2px radius and half
//! a pixel of border, and at that ratio a terminal cell is enough resolution to
//! draw it properly — anti-aliased, square whatever the font's aspect ratio, and in
//! the same Primer green.
//!
//! Two protocols, one image:
//!
//! - **kitty** takes RGBA with an alpha channel, so the rounded corners blend into
//!   whatever the terminal's background is — an image, a gradient, transparency.
//!   The payload is zlib'd, which takes a year of flat green from half a megabyte
//!   to a few kilobytes.
//! - **sixel** has no alpha, only "leave this pixel alone", so edges are composited
//!   against the background colour the terminal reported and everything outside a
//!   cell is left untouched. Its palette is 8-bit, so the anti-aliased blends are
//!   pooled into a palette that fits, coarsening only as far as it has to.
//!
//! What the two share is that the grid lands on exact character-cell boundaries:
//! two columns and one row per day. That is what lets the month labels stay in
//! text, the mouse hit-test stay integer arithmetic, and a hovered day be repainted
//! on its own without redrawing the year.

use std::collections::HashMap;
use std::io::{self, Write};

use crate::primer::{Palette, Rgb, EDGE_ALPHA};

/// Columns one day occupies. A character cell is about twice as tall as it is wide,
/// so two of them is the nearest thing to a square the grid offers — and the same
/// stride the two-column text styles use, so the mouse maths is shared.
pub const COLUMNS_PER_DAY: u16 = 2;

/// github.com's geometry, as ratios so it scales to whatever a cell measures:
/// an 11px square on a 14px pitch, 2px corner radius, 0.5px border.
const SQUARE_OF_PITCH: f32 = 11.0 / 14.0;
const RADIUS_OF_SQUARE: f32 = 2.0 / 11.0;
const BORDER_OF_SQUARE: f32 = 0.5 / 11.0;
/// `outline: 2px solid; outline-offset: -1px` on an 11px cell, for the focused day:
/// a stroke centred on the square's edge, half of it over the cell and half of it
/// in the gap, so the day underneath stays the colour it was.
const RING_OF_SQUARE: f32 = 2.0 / 11.0;

/// Which protocol the terminal speaks. Resolved once, at startup.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Protocol {
    /// The kitty graphics protocol: RGBA with an alpha channel, zlib'd.
    Kitty,
    /// Sixel: a palette, six pixels to a byte, and no transparency but "skip".
    Sixel,
}

impl Protocol {
    /// The name shown beside the legend.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Kitty => "kitty",
            Self::Sixel => "sixel",
        }
    }
}

// ---------------------------------------------------------------- raster

/// A straight-alpha RGBA bitmap.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Image {
    /// Width in pixels.
    pub width: usize,
    /// Height in pixels.
    pub height: usize,
    pixels: Vec<[u8; 4]>,
}

impl Image {
    /// A fully transparent image.
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            pixels: vec![[0; 4]; width * height],
        }
    }

    /// A rounded rectangle, anti-aliased from the signed distance to its edge:
    /// coverage is how much of the pixel's square falls inside, approximated as
    /// `0.5 - distance` and clamped. One evaluation per pixel, no supersampling,
    /// and exact enough that a 2px radius reads as a 2px radius.
    pub fn rounded_rect(&mut self, x: f32, y: f32, side: f32, radius: f32, color: Rgb, alpha: f32) {
        let radius = radius.clamp(0.0, side / 2.0);
        let (cx, cy) = (x + side / 2.0, y + side / 2.0);
        let half = side / 2.0 - radius;

        let low = |v: f32| (v.floor() as isize).max(0);
        let high = |v: f32, limit: usize| (v.ceil() as isize + 1).min(limit as isize);
        for py in low(y)..high(y + side, self.height) {
            for px in low(x)..high(x + side, self.width) {
                let dx = (px as f32 + 0.5 - cx).abs() - half;
                let dy = (py as f32 + 0.5 - cy).abs() - half;
                let outside = dx.max(0.0).hypot(dy.max(0.0));
                let distance = outside + dx.max(dy).min(0.0) - radius;
                let coverage = (0.5 - distance).clamp(0.0, 1.0) * alpha;
                if coverage > 0.0 {
                    self.blend(px as usize, py as usize, color, coverage);
                }
            }
        }
    }

    /// A rounded-rect outline `width` thick, from the same signed distance as
    /// [`Image::rounded_rect`] so it lands in exactly the same place: the annulus
    /// between the outer edge and that edge brought in by `width`.
    ///
    /// A filled rect cannot draw this, and a day that has not happened has no
    /// square to put a ring around — but it can still hold the cursor, and the
    /// character styles draw one there.
    pub fn rounded_ring(
        &mut self,
        at: (f32, f32),
        side: f32,
        radius: f32,
        width: f32,
        color: Rgb,
        alpha: f32,
    ) {
        let (x, y) = at;
        let coverage = |px: isize, py: isize, x: f32, y: f32, side: f32, radius: f32| -> f32 {
            let radius = radius.clamp(0.0, side / 2.0);
            let (cx, cy) = (x + side / 2.0, y + side / 2.0);
            let half = side / 2.0 - radius;
            let dx = (px as f32 + 0.5 - cx).abs() - half;
            let dy = (py as f32 + 0.5 - cy).abs() - half;
            let outside = dx.max(0.0).hypot(dy.max(0.0));
            let distance = outside + dx.max(dy).min(0.0) - radius;
            (0.5 - distance).clamp(0.0, 1.0)
        };
        let low = |v: f32| (v.floor() as isize).max(0);
        let high = |v: f32, limit: usize| (v.ceil() as isize + 1).min(limit as isize);
        let inner = (side - 2.0 * width).max(0.0);
        for py in low(y)..high(y + side, self.height) {
            for px in low(x)..high(x + side, self.width) {
                let outer = coverage(px, py, x, y, side, radius);
                let hole = coverage(
                    px,
                    py,
                    x + width,
                    y + width,
                    inner,
                    (radius - width).max(0.0),
                );
                let ring = (outer - hole).clamp(0.0, 1.0) * alpha;
                if ring > 0.0 {
                    self.blend(px as usize, py as usize, color, ring);
                }
            }
        }
    }

    /// `color` over whatever is already there, straight alpha.
    fn blend(&mut self, x: usize, y: usize, color: Rgb, alpha: f32) {
        let under = self.pixels[y * self.width + x];
        let below = f32::from(under[3]) / 255.0;
        let out = alpha + below * (1.0 - alpha);
        let mix = |src: u8, dst: u8| {
            if out <= 0.0 {
                return 0;
            }
            let value = (f32::from(src) * alpha + f32::from(dst) * below * (1.0 - alpha)) / out;
            value.round().clamp(0.0, 255.0) as u8
        };
        self.pixels[y * self.width + x] = [
            mix(color.0, under[0]),
            mix(color.1, under[1]),
            mix(color.2, under[2]),
            (out * 255.0).round().clamp(0.0, 255.0) as u8,
        ];
    }

    /// One pixel, straight RGBA.
    pub fn rgba_at(&self, x: usize, y: usize) -> [u8; 4] {
        self.pixels[y * self.width + x]
    }

    fn rgba(&self) -> Vec<u8> {
        self.pixels.iter().flatten().copied().collect()
    }

    /// True when nothing was ever drawn — an empty year, or a cell off the end of it.
    pub fn is_blank(&self) -> bool {
        self.pixels.iter().all(|pixel| pixel[3] == 0)
    }
}

/// How a day is marked out from its neighbours.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ring {
    /// The keyboard cursor: `--fgColor-default`, the way a focused cell outlines.
    Cursor,
    /// The day under the mouse: `--fgColor-accent`.
    Hover,
}

impl Ring {
    const fn slot(self) -> usize {
        match self {
            Self::Cursor => 0,
            Self::Hover => 1,
        }
    }

    fn color(self, palette: &Palette) -> Rgb {
        match self {
            Self::Cursor => palette.fg,
            Self::Hover => palette.accent,
        }
    }
}

/// One day's square, centred in its two-column pitch.
fn square(cell: (u16, u16)) -> (f32, f32, f32) {
    let pitch_w = f32::from(cell.0) * f32::from(COLUMNS_PER_DAY);
    let pitch_h = f32::from(cell.1);
    // Square regardless of the font's aspect ratio: the pitch is only square when a
    // cell is exactly twice as tall as it is wide, so take the smaller side.
    let side = (pitch_w.min(pitch_h) * SQUARE_OF_PITCH).round().max(3.0);
    // Rounded, like the side: a half-pixel offset made the square soft on one
    // axis and crisp on the other — at a 9x19 cell, an opaque extent of 12x11
    // where 12x12 was drawn. Where the slack is odd one edge gets the extra
    // pixel, which is a pixel of asymmetry instead of two of blur.
    (
        ((pitch_w - side) / 2.0).round(),
        ((pitch_h - side) / 2.0).round(),
        side,
    )
}

/// Draw one day at `(x, y)` in image space.
fn day(
    image: &mut Image,
    x: f32,
    y: f32,
    side: f32,
    fill: Rgb,
    palette: &Palette,
    ring: Option<Ring>,
) {
    let radius = side * RADIUS_OF_SQUARE;
    match ring {
        Some(ring) => {
            let width = (side * RING_OF_SQUARE).max(1.0);
            let half = width / 2.0;
            image.rounded_rect(
                x - half,
                y - half,
                side + width,
                radius + half,
                ring.color(palette),
                1.0,
            );
            image.rounded_rect(x + half, y + half, side - width, radius - half, fill, 1.0);
        }
        None => {
            // The square at its full size, then the hairline border *over* its
            // edge. Inset instead, the border shrank every coloured cell: at a
            // 10x20 cell the green measured 14px in a 20px pitch — 0.700 — where
            // github.com's is 11 on 14, or 0.786. A 5%-alpha border only reads as
            // a border when it is composited over something; over the terminal
            // background it was invisible, so the cell simply looked smaller.
            let border = (side * BORDER_OF_SQUARE).max(0.5);
            image.rounded_rect(x, y, side, radius, fill, 1.0);
            image.rounded_ring((x, y), side, radius, border, palette.edge, EDGE_ALPHA);
        }
    }
}

/// The whole year. `levels[week][weekday]` is `None` for a day outside the year or
/// one still to come, both of which draw nothing at all.
pub fn grid(levels: &[[Option<u8>; 7]], palette: &Palette, cell: (u16, u16)) -> Image {
    let (dx, dy, side) = square(cell);
    let mut image = Image::new(
        levels.len() * usize::from(cell.0) * usize::from(COLUMNS_PER_DAY),
        7 * usize::from(cell.1),
    );
    for (week, column) in levels.iter().enumerate() {
        for (weekday, level) in column.iter().enumerate() {
            let Some(level) = level else { continue };
            let x = (week * usize::from(cell.0) * usize::from(COLUMNS_PER_DAY)) as f32 + dx;
            let y = (weekday * usize::from(cell.1)) as f32 + dy;
            day(&mut image, x, y, side, palette.cell(*level), palette, None);
        }
    }
    image
}

/// The five legend swatches, drawn exactly as the chart draws a day.
pub fn legend(palette: &Palette, cell: (u16, u16)) -> Image {
    grid(
        &(0..5)
            .map(|level| {
                let mut column = [None; 7];
                column[0] = Some(level as u8);
                column
            })
            .collect::<Vec<_>>(),
        palette,
        cell,
    )
    .crop_rows(usize::from(cell.1))
}

impl Image {
    /// Keep the top `rows` pixels. The legend reuses the grid's layout, which is
    /// seven weekdays tall, and wants only the first.
    fn crop_rows(mut self, rows: usize) -> Self {
        let rows = rows.min(self.height);
        self.pixels.truncate(rows * self.width);
        self.height = rows;
        self
    }
}

/// One day on its own, for repainting a single cell when the cursor or the mouse
/// moves. Same geometry as the grid, so it lands exactly over the cell it replaces.
pub fn patch(level: Option<u8>, ring: Option<Ring>, palette: &Palette, cell: (u16, u16)) -> Image {
    let (dx, dy, side) = square(cell);
    let mut image = Image::new(
        usize::from(cell.0) * usize::from(COLUMNS_PER_DAY),
        usize::from(cell.1),
    );
    match (level, ring) {
        (Some(level), ring) => day(&mut image, dx, dy, side, palette.cell(level), palette, ring),
        // No square to draw, but the cursor can be walked into the future and the
        // detail line names the day it is on. Drawing nothing at all left it
        // invisible in pixel mode while every bordered style showed it.
        (None, Some(ring)) => {
            let width = (side * RING_OF_SQUARE).max(1.0);
            image.rounded_ring(
                (dx - width / 2.0, dy - width / 2.0),
                side + width,
                side * RADIUS_OF_SQUARE + width / 2.0,
                width,
                ring.color(palette),
                1.0,
            );
        }
        (None, None) => {}
    }
    image
}

// ---------------------------------------------------------------- kitty

/// Ids are ours to choose; keeping them fixed means a redraw replaces an image
/// rather than piling another one on top of it.
const BASE_ID: u32 = 7380;
const LEGEND_ID: u32 = 7381;
const RING_ID: u32 = 7382;

/// Drawn below text, above the cell background, so a tooltip can sit over the chart
/// without punching a hole in it. Rings go one layer above the year.
const Z_GRID: i32 = -2;
const Z_RING: i32 = -1;

/// Transmit and place, in one go. `columns`/`rows` pin the placement to exactly the
/// character cells the layout reserved, so the image cannot drift out of step with
/// the month labels even if the terminal's idea of a cell disagrees with ours.
pub fn kitty(image: &Image, id: u32, columns: u16, rows: u16, z: i32) -> String {
    let payload = base64(&miniz_oxide::deflate::compress_to_vec_zlib(
        &image.rgba(),
        6,
    ));
    let control = format!(
        "a=T,q=2,f=32,o=z,s={},v={},i={id},p=1,z={z},c={columns},r={rows},C=1",
        image.width, image.height
    );

    // 4096 bytes of payload per escape is the protocol's limit.
    let mut out = String::with_capacity(payload.len() + payload.len() / 32 + 64);
    let mut chunks = payload.as_bytes().chunks(4096).peekable();
    let mut first = true;
    while let Some(chunk) = chunks.next() {
        let more = u8::from(chunks.peek().is_some());
        out.push_str("\x1b_G");
        if first {
            out.push_str(&control);
            out.push(',');
            first = false;
        }
        out.push_str(&format!("m={more};"));
        out.push_str(std::str::from_utf8(chunk).unwrap_or_default());
        out.push_str("\x1b\\");
    }
    out
}

/// Delete an image and everything drawn from it.
pub fn kitty_delete(id: u32) -> String {
    format!("\x1b_Ga=d,d=I,i={id},q=2\x1b\\")
}

fn base64(data: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for group in data.chunks(3) {
        let mut bits = 0u32;
        for (index, byte) in group.iter().enumerate() {
            bits |= u32::from(*byte) << (16 - 8 * index);
        }
        for index in 0..=group.len() {
            out.push(ALPHABET[(bits >> (18 - 6 * index) & 0x3f) as usize] as char);
        }
        for _ in group.len()..3 {
            out.push('=');
        }
    }
    out
}

// ---------------------------------------------------------------- sixel

/// Sixel colours are percentages, and there are only 256 registers. Anti-aliasing a
/// year of cells produces far more shades than that, so the blends are snapped to a
/// coarser grid until they fit — starting at full precision, which is usually
/// enough, and only losing accuracy on a palette that would not have fit anyway.
fn indexed(image: &Image, background: Rgb) -> (Vec<[u8; 3]>, Vec<Option<u8>>) {
    /// Registers `#0`-`#255`, so all 256 are addressable. Checked *before* an
    /// index is handed out: `seen.len() as u8` silently wraps to 0 on the
    /// 257th colour, and an index that aliases another colour must never be
    /// constructed, whether or not the attempt it belongs to is discarded.
    const REGISTERS: usize = 256;

    for step in [1u16, 2, 3, 4, 6, 20] {
        let mut palette: Vec<[u8; 3]> = Vec::new();
        let mut seen: HashMap<[u8; 3], u8> = HashMap::new();
        let mut map = Vec::with_capacity(image.width * image.height);
        let mut overflowed = false;

        for pixel in &image.pixels {
            if pixel[3] == 0 {
                map.push(None);
                continue;
            }
            let alpha = f32::from(pixel[3]) / 255.0;
            let solid = Rgb(pixel[0], pixel[1], pixel[2]).over(background, alpha);
            let snap = |value: u8| {
                let percent = (u16::from(value) * 100 + 127) / 255;
                ((percent + step / 2) / step * step).min(100) as u8
            };
            let key = [snap(solid.0), snap(solid.1), snap(solid.2)];
            let index = match seen.get(&key) {
                Some(index) => *index,
                None if palette.len() == REGISTERS => {
                    overflowed = true;
                    break;
                }
                None => {
                    let index = palette.len() as u8;
                    palette.push(key);
                    seen.insert(key, index);
                    index
                }
            };
            map.push(Some(index));
        }
        if !overflowed {
            return (palette, map);
        }
    }
    unreachable!("20% steps leave 6³ = 216 shades, which fits, and cells use far fewer")
}

/// `P2 = 1`: a pixel nobody paints is left as it was, which is how a rounded corner
/// stays rounded over a terminal background sixel cannot see.
pub fn sixel(image: &Image, background: Rgb) -> String {
    let (palette, map) = indexed(image, background);
    let mut out = String::from("\x1bP0;1;0q");
    out.push_str(&format!("\"1;1;{};{}", image.width, image.height));
    for (index, color) in palette.iter().enumerate() {
        out.push_str(&format!(
            "#{index};2;{};{};{}",
            color[0], color[1], color[2]
        ));
    }

    for top in (0..image.height).step_by(6) {
        // One pass over the band collects every colour in it, so the per-colour
        // passes that follow are the only time the width is walked twice.
        let mut bands: HashMap<u8, Vec<u8>> = HashMap::new();
        for row in 0..6 {
            let y = top + row;
            if y >= image.height {
                break;
            }
            for x in 0..image.width {
                if let Some(index) = map[y * image.width + x] {
                    bands.entry(index).or_insert_with(|| vec![0; image.width])[x] |= 1 << row;
                }
            }
        }

        let mut indices: Vec<&u8> = bands.keys().collect();
        indices.sort_unstable();
        for index in indices {
            let bits = &bands[index];
            let last = bits.iter().rposition(|byte| *byte != 0);
            let Some(last) = last else { continue };
            out.push_str(&format!("#{index}"));
            let (mut run, mut code) = (0usize, bits[0]);
            for byte in &bits[..=last] {
                if *byte == code {
                    run += 1;
                } else {
                    push_run(&mut out, code, run);
                    (run, code) = (1, *byte);
                }
            }
            push_run(&mut out, code, run);
            out.push('$');
        }
        out.push('-');
    }
    out.push_str("\x1b\\");
    out
}

fn push_run(out: &mut String, code: u8, count: usize) {
    let glyph = char::from(b'?' + code);
    if count >= 4 {
        out.push('!');
        out.push_str(&count.to_string());
        out.push(glyph);
    } else {
        for _ in 0..count {
            out.push(glyph);
        }
    }
}

// ---------------------------------------------------------------- painting

/// A day to be marked, in grid coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Mark {
    /// Column in the grid.
    pub week: u16,
    /// Row in the grid, 0 = Sunday.
    pub weekday: u16,
    /// The day's shade, or `None` where there is no day to draw.
    pub level: Option<u8>,
    /// Which ring to draw around it.
    pub ring: Ring,
}

/// Everything the painter needs for one frame. Built by the app, which knows where
/// the layout put the grid; the painter only diffs it against what is on screen.
#[derive(Debug)]
pub struct Scene<'a> {
    /// The colours to draw with.
    pub palette: &'a Palette,
    /// Screen position of the top-left cell of the grid.
    pub grid: (u16, u16),
    /// Screen position of the legend swatches, when they are on screen.
    pub legend: Option<(u16, u16)>,
    /// `levels[week][weekday]`, `None` where there is no day to draw.
    pub levels: Vec<[Option<u8>; 7]>,
    /// The cursor and the hovered day: at most one of each, never both on the same
    /// cell — two rings there would draw over each other, and erasing one would
    /// take the other with it. Order does not matter; each mark carries its ring.
    pub marks: [Option<Mark>; 2],
    /// Changes when the image would differ: year, user, palette, cell size.
    pub key: u64,
}

/// What is currently on the screen, so a frame only writes what changed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Painted {
    key: u64,
    grid: (u16, u16),
    legend: Option<(u16, u16)>,
}

/// Keeps the screen's pixels in step with the chart, writing only what changed.
#[derive(Debug)]
pub struct Painter {
    /// The protocol this terminal speaks.
    pub protocol: Protocol,
    /// One character cell in pixels, which sets the image scale.
    pub cell: (u16, u16),
    /// Columns the terminal has, so clearing cells for a sixel cannot write past
    /// the last one and wrap over the rows below.
    pub width: u16,
    background: Rgb,
    base: Option<Painted>,
    marks: [Option<Mark>; 2],
}

impl Painter {
    /// A painter for a terminal that speaks `protocol`, with cells of `cell`
    /// pixels over `background`.
    pub fn new(protocol: Protocol, cell: (u16, u16), background: Rgb) -> Self {
        Self {
            protocol,
            cell,
            width: u16::MAX,
            background,
            base: None,
            marks: [None; 2],
        }
    }

    /// Forget what is on screen. The next paint redraws from nothing — after a
    /// resize, a clear, or anything else that wipes the terminal under us.
    pub fn invalidate(&mut self) {
        self.base = None;
        self.marks = [None; 2];
    }

    /// Take the chart off the screen: kitty holds images until told otherwise, so
    /// leaving them behind would ghost over whatever is drawn next.
    pub fn clear(&mut self, out: &mut impl Write) -> io::Result<()> {
        if self.base.is_none() && self.marks.iter().all(Option::is_none) {
            return Ok(());
        }
        if self.protocol == Protocol::Kitty {
            for id in [BASE_ID, LEGEND_ID, RING_ID, RING_ID + 1] {
                out.write_all(kitty_delete(id).as_bytes())?;
            }
            out.flush()?;
        }
        self.invalidate();
        Ok(())
    }

    /// Bring the screen up to date with `scene`, writing only what changed.
    pub fn paint(&mut self, out: &mut impl Write, scene: &Scene<'_>) -> io::Result<()> {
        let wanted = Painted {
            key: scene.key,
            grid: scene.grid,
            legend: scene.legend,
        };
        if self.base != Some(wanted) {
            self.draw_base(out, scene)?;
            self.base = Some(wanted);
            // A fresh year carries no rings: whatever was on the old one is gone.
            self.marks = [None; 2];
        }

        // A mark's ring decides which slot it holds, so a caller cannot put them in
        // the wrong order and have rings deleted by the wrong id.
        let mut wanted: [Option<Mark>; 2] = [None; 2];
        for mark in scene.marks.iter().flatten() {
            wanted[mark.ring.slot()] = Some(*mark);
        }

        for (slot, new) in wanted.into_iter().enumerate() {
            let old = self.marks[slot];
            if old == new {
                continue;
            }
            if let Some(old) = old {
                self.erase(out, scene, old)?;
            }
            if let Some(new) = new {
                self.mark(out, scene, new)?;
            }
            self.marks[slot] = new;
        }
        out.flush()
    }

    fn draw_base(&mut self, out: &mut impl Write, scene: &Scene<'_>) -> io::Result<()> {
        let image = grid(&scene.levels, scene.palette, self.cell);
        self.blank(
            out,
            scene.grid,
            scene.levels.len() as u16 * COLUMNS_PER_DAY,
            7,
        )?;
        self.place(out, scene.grid, &image, BASE_ID, Z_GRID)?;
        if let Some(at) = scene.legend {
            let image = legend(scene.palette, self.cell);
            self.blank(out, at, 5 * COLUMNS_PER_DAY, 1)?;
            self.place(out, at, &image, LEGEND_ID, Z_GRID)?;
        }
        Ok(())
    }

    fn mark(&mut self, out: &mut impl Write, scene: &Scene<'_>, mark: Mark) -> io::Result<()> {
        let image = patch(mark.level, Some(mark.ring), scene.palette, self.cell);
        let at = self.cell_at(scene, mark);
        let id = RING_ID + mark.ring.slot() as u32;
        self.place(out, at, &image, id, Z_RING)
    }

    /// Put a marked day back the way the year drew it. Kitty only has to drop the
    /// ring, since the year is still underneath; sixel has to paint the day again.
    fn erase(&mut self, out: &mut impl Write, scene: &Scene<'_>, mark: Mark) -> io::Result<()> {
        match self.protocol {
            Protocol::Kitty => {
                let id = RING_ID + mark.ring.slot() as u32;
                out.write_all(kitty_delete(id).as_bytes())
            }
            Protocol::Sixel => {
                // Spaces first. A ring is drawn half in the gap between cells, so
                // repainting only the square would leave its outer edge behind —
                // and blanking the two character cells restores the terminal's real
                // background, which is better than repainting the one we guessed.
                let at = self.cell_at(scene, mark);
                write!(
                    out,
                    "\x1b[{};{}H\x1b[0m{}",
                    at.1 + 1,
                    at.0 + 1,
                    " ".repeat(usize::from(COLUMNS_PER_DAY))
                )?;
                let image = patch(mark.level, None, scene.palette, self.cell);
                if image.is_blank() {
                    return Ok(());
                }
                self.place(out, at, &image, RING_ID, Z_RING)
            }
        }
    }

    /// Wipe the character cells an image is about to cover. Only sixel needs it,
    /// and only when redrawing: a day that is gone from the new year leaves nothing
    /// behind it, and "nothing" in sixel means the old pixels stay.
    fn blank(
        &self,
        out: &mut impl Write,
        at: (u16, u16),
        columns: u16,
        rows: u16,
    ) -> io::Result<()> {
        if self.protocol != Protocol::Sixel {
            return Ok(());
        }
        for row in 0..rows {
            write!(
                out,
                "\x1b[{};{}H\x1b[0m{}",
                at.1 + row + 1,
                at.0 + 1,
                " ".repeat(usize::from(columns.min(self.width.saturating_sub(at.0))))
            )?;
        }
        Ok(())
    }

    fn cell_at(&self, scene: &Scene<'_>, mark: Mark) -> (u16, u16) {
        (
            scene.grid.0 + mark.week * COLUMNS_PER_DAY,
            scene.grid.1 + mark.weekday,
        )
    }

    /// Draw `image` with its top-left corner on the character cell `at`. Every
    /// image here is a whole number of cells wide and tall, so how many it covers
    /// is measured rather than passed — kitty is told, and then cannot disagree.
    fn place(
        &self,
        out: &mut impl Write,
        at: (u16, u16),
        image: &Image,
        id: u32,
        z: i32,
    ) -> io::Result<()> {
        let payload = match self.protocol {
            Protocol::Kitty => {
                let columns = image.width.div_ceil(usize::from(self.cell.0)) as u16;
                let rows = image.height.div_ceil(usize::from(self.cell.1)) as u16;
                kitty(image, id, columns, rows, z)
            }
            Protocol::Sixel => sixel(image, self.background),
        };
        // Absolute placement every time: the cursor is wherever the last frame's
        // text left it, and both protocols draw from wherever it is.
        write!(out, "\x1b[{};{}H{payload}", at.1 + 1, at.0 + 1)
    }
}
