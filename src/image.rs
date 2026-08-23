//! Turning a picture into a year: decode a PNG, shrink it to the calendar, and
//! quantise it to GitHub's five shades.
//!
//! Small on purpose, and for the same reason [`crate::png`] is: the encoder
//! there needed one colour type and one filter, and this needs the handful a
//! real file actually uses. zlib comes from the compressor the kitty protocol
//! already pulls in, so decoding a PNG costs this crate no new dependency at
//! all — which matters more here than breadth, because the alternative is a
//! general-purpose image library and its transitive tree for one flag.
//!
//! What it does not do is say so plainly. An interlaced PNG, a 1/2/4-bit
//! palette, a JPEG — each is refused by name rather than half-decoded into a
//! picture nobody drew.
//!
//! # The direction of the mapping
//!
//! A dark pixel becomes a *bright* day. Contribution art is ink on an empty
//! field: black on white in the source is the densest green on the graph, the
//! way a drawing on paper reads. `invert` turns it over for a picture that was
//! light on dark to begin with.

use crate::art::{Canvas, CANVAS_COLS, CANVAS_ROWS};

/// A decoded image: 8-bit RGBA, row-major.
#[derive(Debug, Clone)]
pub struct Bitmap {
    /// Pixels across.
    pub width: usize,
    /// Pixels down.
    pub height: usize,
    /// `width * height * 4` bytes, RGBA.
    pub pixels: Vec<u8>,
}

impl Bitmap {
    /// The pixel at a position, or opaque white for one off the image.
    ///
    /// White rather than black or transparent, because it is the value that
    /// makes padding disappear: the edges of an image that does not fill the
    /// calendar should read as empty days, and white is what maps to level 0.
    #[must_use]
    pub fn at(&self, x: usize, y: usize) -> [u8; 4] {
        if x >= self.width || y >= self.height {
            return [255, 255, 255, 255];
        }
        let base = (y * self.width + x) * 4;
        self.pixels
            .get(base..base + 4)
            .map_or([255, 255, 255, 255], |slice| {
                [slice[0], slice[1], slice[2], slice[3]]
            })
    }
}

/// How a picture should be turned into shades.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Options {
    /// Map bright pixels to bright days instead of dark ones.
    pub invert: bool,
    /// Spread the quantisation error into neighbouring days, so a gradient
    /// reads as one rather than as four bands.
    pub dither: bool,
}

/// Decode a PNG, or say why it could not be decoded.
///
/// Supports the colour types and bit depths a file in the wild actually
/// carries: 8- and 16-bit greyscale, truecolour, indexed and both alpha
/// variants. 16-bit samples are taken by their high byte, which is what
/// quantising to five shades would do to them anyway.
pub fn decode_png(bytes: &[u8]) -> Result<Bitmap, String> {
    if bytes.len() < 8 || &bytes[..8] != b"\x89PNG\r\n\x1a\n" {
        return Err(describe_format(bytes));
    }

    let mut cursor = 8;
    let (mut width, mut height) = (0usize, 0usize);
    let (mut depth, mut colour, mut interlace) = (0u8, 0u8, 0u8);
    let mut palette: Vec<[u8; 3]> = Vec::new();
    let mut alphas: Vec<u8> = Vec::new();
    let mut idat: Vec<u8> = Vec::new();
    let mut seen_header = false;

    while cursor + 8 <= bytes.len() {
        let length = u32::from_be_bytes(
            bytes[cursor..cursor + 4]
                .try_into()
                .map_err(|_| "truncated chunk length".to_string())?,
        ) as usize;
        let kind = &bytes[cursor + 4..cursor + 8];
        let start = cursor + 8;
        let end = start
            .checked_add(length)
            .filter(|end| *end <= bytes.len())
            .ok_or_else(|| "a chunk runs past the end of the file".to_string())?;
        let body = &bytes[start..end];

        match kind {
            b"IHDR" => {
                if body.len() < 13 {
                    return Err("the header chunk is too short".to_string());
                }
                width = u32::from_be_bytes(body[0..4].try_into().expect("four bytes")) as usize;
                height = u32::from_be_bytes(body[4..8].try_into().expect("four bytes")) as usize;
                depth = body[8];
                colour = body[9];
                interlace = body[12];
                seen_header = true;
            }
            b"PLTE" => {
                // `as_chunks` rather than `chunks_exact`: the entries are
                // fixed-size by definition, and saying so gives arrays back
                // instead of slices that have to be re-indexed.
                let (entries, _partial) = body.as_chunks::<3>();
                palette = entries.to_vec();
            }
            b"tRNS" => alphas = body.to_vec(),
            b"IDAT" => idat.extend_from_slice(body),
            b"IEND" => break,
            _ => {}
        }
        // 4 length + 4 kind + body + 4 CRC. The CRC is not checked: a file that
        // decodes into a picture is the evidence that matters here, and a
        // mismatched CRC on a file someone is trying to draw would refuse work
        // that would otherwise have succeeded.
        cursor = end + 4;
    }

    if !seen_header {
        return Err("no PNG header chunk".to_string());
    }
    if width == 0 || height == 0 {
        return Err("the image is zero-sized".to_string());
    }
    if interlace != 0 {
        return Err(
            "this PNG is interlaced (Adam7), which is not supported — re-save it \
             without interlacing"
                .to_string(),
        );
    }
    if !matches!(depth, 8 | 16) {
        return Err(format!(
            "this PNG is {depth} bits per sample; 8 and 16 are supported — re-save \
             it at 8 bits"
        ));
    }
    let channels = match colour {
        0 => 1, // greyscale
        2 => 3, // truecolour
        3 => 1, // indexed
        4 => 2, // greyscale + alpha
        6 => 4, // truecolour + alpha
        other => return Err(format!("colour type {other} is not one PNG defines")),
    };
    if colour == 3 && palette.is_empty() {
        return Err("an indexed PNG with no palette".to_string());
    }

    let raw = miniz_oxide::inflate::decompress_to_vec_zlib(&idat)
        .map_err(|error| format!("the image data would not inflate: {error:?}"))?;

    let sample = usize::from(depth) / 8;
    let stride = width * channels * sample;
    let step = channels * sample;
    let mut previous = vec![0u8; stride];
    let mut pixels = Vec::with_capacity(width * height * 4);

    for row in 0..height {
        let line = row
            .checked_mul(stride + 1)
            .and_then(|base| raw.get(base..base + stride + 1))
            .ok_or_else(|| format!("the image data is short: row {row} of {height} is missing"))?;
        let filter = line[0];
        let mut current = line[1..].to_vec();
        unfilter(filter, &mut current, &previous, step)?;

        for x in 0..width {
            let base = x * step;
            let take = |index: usize| current.get(base + index * sample).copied().unwrap_or(0);
            let rgba = match colour {
                0 => {
                    let grey = take(0);
                    [grey, grey, grey, 255]
                }
                2 => [take(0), take(1), take(2), 255],
                3 => {
                    let index = usize::from(take(0));
                    let rgb = palette.get(index).copied().unwrap_or([0, 0, 0]);
                    let alpha = alphas.get(index).copied().unwrap_or(255);
                    [rgb[0], rgb[1], rgb[2], alpha]
                }
                4 => {
                    let grey = take(0);
                    [grey, grey, grey, take(1)]
                }
                _ => [take(0), take(1), take(2), take(3)],
            };
            pixels.extend_from_slice(&rgba);
        }
        previous = current;
    }

    Ok(Bitmap {
        width,
        height,
        pixels,
    })
}

/// Undo one PNG row filter in place.
///
/// Indexed rather than iterated, and clippy is told so deliberately: three of
/// the five filters read bytes of *this* row that the loop has already
/// rewritten — `Sub` reads one pixel back, `Average` and `Paeth` read back and
/// up — so the loop body needs both the current index and earlier, mutated
/// values. `split_at_mut` gymnastics to express that as an iterator would make
/// the arithmetic harder to check against the specification, which is the one
/// thing this function has to be right about.
#[allow(clippy::needless_range_loop)]
fn unfilter(filter: u8, row: &mut [u8], previous: &[u8], step: usize) -> Result<(), String> {
    let left = |row: &[u8], index: usize| -> i32 {
        if index >= step {
            i32::from(row[index - step])
        } else {
            0
        }
    };
    match filter {
        0 => {}
        1 => {
            for index in 0..row.len() {
                let a = left(row, index);
                row[index] = (i32::from(row[index]) + a) as u8;
            }
        }
        2 => {
            for index in 0..row.len() {
                let b = i32::from(previous.get(index).copied().unwrap_or(0));
                row[index] = (i32::from(row[index]) + b) as u8;
            }
        }
        3 => {
            for index in 0..row.len() {
                let a = left(row, index);
                let b = i32::from(previous.get(index).copied().unwrap_or(0));
                row[index] = (i32::from(row[index]) + (a + b) / 2) as u8;
            }
        }
        4 => {
            for index in 0..row.len() {
                let a = left(row, index);
                let b = i32::from(previous.get(index).copied().unwrap_or(0));
                let c = if index >= step {
                    i32::from(previous.get(index - step).copied().unwrap_or(0))
                } else {
                    0
                };
                row[index] = (i32::from(row[index]) + paeth(a, b, c)) as u8;
            }
        }
        other => return Err(format!("row filter {other} is not one PNG defines")),
    }
    Ok(())
}

/// PNG's Paeth predictor: whichever of left, above and above-left is closest to
/// their linear estimate.
fn paeth(a: i32, b: i32, c: i32) -> i32 {
    let estimate = a + b - c;
    let (da, db, dc) = (
        (estimate - a).abs(),
        (estimate - b).abs(),
        (estimate - c).abs(),
    );
    if da <= db && da <= dc {
        a
    } else if db <= dc {
        b
    } else {
        c
    }
}

/// Name the format, so a refusal teaches rather than just declines.
fn describe_format(bytes: &[u8]) -> String {
    let named = match bytes {
        [0xFF, 0xD8, 0xFF, ..] => Some("a JPEG"),
        [b'R', b'I', b'F', b'F', _, _, _, _, b'W', b'E', b'B', b'P', ..] => Some("a WebP"),
        [b'G', b'I', b'F', b'8', ..] => Some("a GIF"),
        [b'B', b'M', ..] => Some("a BMP"),
        _ => None,
    };
    match named {
        Some(what) => format!(
            "this is {what}, and --image reads PNG.\n  \
             Converting it costs nothing and loses nothing at this size:\n    \
             magick input -strip output.png     (or any image editor's \"save as\")"
        ),
        None => "this is not a PNG — --image reads PNG".to_string(),
    }
}

/// How bright a pixel is, 0.0 for black and 1.0 for white, composited over
/// white so that a transparent background reads as empty rather than as ink.
fn luminance(rgba: [u8; 4]) -> f32 {
    let alpha = f32::from(rgba[3]) / 255.0;
    let over = |channel: u8| (f32::from(channel) * alpha + 255.0 * (1.0 - alpha)) / 255.0;
    // Rec. 709 luma weights, which is what "how bright does this look" means
    // for sRGB values.
    0.2126 * over(rgba[0]) + 0.7152 * over(rgba[1]) + 0.0722 * over(rgba[2])
}

/// Shrink `bitmap` to at most [`CANVAS_COLS`] by [`CANVAS_ROWS`], keeping its
/// aspect ratio, and report the ink at each cell — 0.0 for none, 1.0 for full.
///
/// Every source pixel that lands in a cell is averaged into it rather than
/// sampled from, because the shrink here is enormous — a 1000-pixel-wide image
/// becomes 53 columns — and point sampling at that ratio keeps one pixel in
/// twenty and throws away the picture.
fn ink_grid(bitmap: &Bitmap, invert: bool) -> (Vec<Vec<f32>>, usize, usize) {
    let scale =
        (CANVAS_COLS as f32 / bitmap.width as f32).min(CANVAS_ROWS as f32 / bitmap.height as f32);
    let cols = ((bitmap.width as f32 * scale).round() as usize).clamp(1, CANVAS_COLS);
    let rows = ((bitmap.height as f32 * scale).round() as usize).clamp(1, CANVAS_ROWS);

    let mut grid = vec![vec![0.0f32; cols]; rows];
    for (row, line) in grid.iter_mut().enumerate() {
        for (col, cell) in line.iter_mut().enumerate() {
            let x0 = col * bitmap.width / cols;
            let x1 = ((col + 1) * bitmap.width / cols).max(x0 + 1);
            let y0 = row * bitmap.height / rows;
            let y1 = ((row + 1) * bitmap.height / rows).max(y0 + 1);

            let mut sum = 0.0;
            let mut count = 0.0;
            for y in y0..y1.min(bitmap.height) {
                for x in x0..x1.min(bitmap.width) {
                    sum += luminance(bitmap.at(x, y));
                    count += 1.0;
                }
            }
            let bright = if count > 0.0 { sum / count } else { 1.0 };
            // Ink, not brightness: a dark pixel is a busy day.
            *cell = if invert { bright } else { 1.0 - bright };
        }
    }
    (grid, cols, rows)
}

/// Turn a decoded image into a canvas.
///
/// The result is centred in the calendar, so an image that does not fill the
/// year sits in the middle of it rather than against the left edge.
pub fn to_canvas(bitmap: &Bitmap, options: Options) -> Result<Canvas, String> {
    let (mut grid, cols, rows) = ink_grid(bitmap, options.invert);

    if options.dither {
        floyd_steinberg(&mut grid, cols, rows);
    }

    let mut canvas = Canvas::blank(cols).ok_or_else(|| {
        format!("the image shrank to {cols} columns, which is not a canvas width")
    })?;
    // Centred vertically: a wide image is a band, and a band belongs in the
    // middle of the week rather than on Sunday.
    let top = (CANVAS_ROWS - rows) / 2;
    for (row, line) in grid.iter().enumerate() {
        for (col, ink) in line.iter().enumerate() {
            canvas.set(col, row + top, quantise(*ink));
        }
    }
    Ok(canvas)
}

/// The shade an ink value lands on, 0 to 4.
fn quantise(ink: f32) -> u8 {
    (ink.clamp(0.0, 1.0) * 4.0).round() as u8
}

/// Spread each cell's rounding error into the neighbours that have not been
/// decided yet, so a gradient reads as a gradient instead of as four bands.
///
/// The classic Floyd–Steinberg weights: 7/16 right, 3/16 down-left, 5/16 down,
/// 1/16 down-right.
fn floyd_steinberg(grid: &mut [Vec<f32>], cols: usize, rows: usize) {
    for row in 0..rows {
        for col in 0..cols {
            let old = grid[row][col];
            let new = f32::from(quantise(old)) / 4.0;
            grid[row][col] = new;
            let error = old - new;
            let mut spill = |r: usize, c: usize, weight: f32| {
                if r < rows && c < cols {
                    grid[r][c] += error * weight;
                }
            };
            if col + 1 < cols {
                spill(row, col + 1, 7.0 / 16.0);
            }
            if row + 1 < rows {
                if col > 0 {
                    spill(row + 1, col - 1, 3.0 / 16.0);
                }
                spill(row + 1, col, 5.0 / 16.0);
                if col + 1 < cols {
                    spill(row + 1, col + 1, 1.0 / 16.0);
                }
            }
        }
    }
}

/// Read an image file and turn it into a canvas.
pub fn load(path: &std::path::Path, options: Options) -> Result<Canvas, String> {
    let bytes = std::fs::read(path)
        .map_err(|error| format!("could not read {}: {error}", path.display()))?;
    let bitmap = decode_png(&bytes).map_err(|why| format!("{}: {why}", path.display()))?;
    let mut canvas = to_canvas(&bitmap, options)?;
    let mut meta = canvas.meta().clone();
    meta.name = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .map(str::to_string);
    meta.description = Some(format!(
        "{}x{} pixels, quantised to five shades{}",
        bitmap.width,
        bitmap.height,
        if options.dither { ", dithered" } else { "" }
    ));
    canvas.set_meta(meta);
    Ok(canvas)
}
