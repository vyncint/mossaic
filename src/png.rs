//! A PNG encoder, so a chart can be looked at without a terminal that draws
//! pixels — the same rasteriser output, in a file anything can open.
//!
//! Small on purpose: one colour type (8-bit RGB), one filter (none), and zlib
//! from the compressor the kitty protocol already needs. Nothing here is a
//! general-purpose image library.

use std::io;
use std::path::Path;

use crate::graphics::Image;
use crate::primer::Rgb;

/// Write `image` composited over `background`, since a chart on a transparent
/// canvas looks like a checkerboard in most viewers.
pub fn write(path: &Path, image: &Image, background: Rgb) -> io::Result<()> {
    let (width, height) = (image.width, image.height);
    let mut raw = Vec::with_capacity(height * (1 + width * 3));
    for y in 0..height {
        raw.push(0); // filter: none
        for x in 0..width {
            let pixel = image.rgba_at(x, y);
            let alpha = f32::from(pixel[3]) / 255.0;
            let over = |channel: u8, under: u8| {
                (f32::from(channel) * alpha + f32::from(under) * (1.0 - alpha)).round() as u8
            };
            raw.push(over(pixel[0], background.0));
            raw.push(over(pixel[1], background.1));
            raw.push(over(pixel[2], background.2));
        }
    }

    let mut out = b"\x89PNG\r\n\x1a\n".to_vec();
    let mut header = Vec::with_capacity(13);
    header.extend((width as u32).to_be_bytes());
    header.extend((height as u32).to_be_bytes());
    header.extend([8, 2, 0, 0, 0]); // 8 bits per channel, truecolour, no interlace
    chunk(&mut out, b"IHDR", &header);
    chunk(
        &mut out,
        b"IDAT",
        &miniz_oxide::deflate::compress_to_vec_zlib(&raw, 6),
    );
    chunk(&mut out, b"IEND", &[]);
    std::fs::write(path, out)
}

fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], body: &[u8]) {
    out.extend((body.len() as u32).to_be_bytes());
    out.extend(kind);
    out.extend(body);
    let mut crc = crc32(kind);
    crc = crc32_continue(crc, body);
    out.extend(crc.to_be_bytes());
}

fn crc32(data: &[u8]) -> u32 {
    crc32_continue(0, data)
}

/// The PNG/zlib CRC, computed a bit at a time rather than from a table: 39
/// bytes of header and one compressed block is not enough data to notice.
fn crc32_continue(previous: u32, data: &[u8]) -> u32 {
    let mut crc = !previous;
    for byte in data {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            crc = if crc & 1 == 1 {
                (crc >> 1) ^ 0xEDB8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}
