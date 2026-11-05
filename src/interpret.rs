//! Draws the image that directives make to convert them back to a sprite.
//!
//! This is a port of what Starbound (and OpenStarbound) actually do to render an image from directives.
//!
//! The engine keeps its rows starting at the bottom, so `Image` does too.
//! Row 0 is the bottom row of the png.
//!
//! Does not port hue, saturation, brightness, fade, scanlines, bicubic scaling.
use anyhow::{Context, Result, anyhow, bail};
use std::collections::HashMap;

pub type Pixel = [u8; 4];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Image {
    pub width: u32,
    pub height: u32,
    /// rows from the bottom
    pub pixels: Vec<Pixel>,
}

impl Image {
    pub fn new(width: u32, height: u32) -> Image {
        Image {
            width,
            height,
            pixels: vec![[0; 4]; width as usize * height as usize],
        }
    }

    pub fn get(&self, x: u32, y: u32) -> Pixel {
        self.pixels[y as usize * self.width as usize + x as usize]
    }

    fn set(&mut self, x: u32, y: u32, pixel: Pixel) {
        self.pixels[y as usize * self.width as usize + x as usize] = pixel;
    }

    /// The pixel at (x, y) with the coordinates held to the image, which an empty image answers with zeroes
    fn clamp(&self, x: i32, y: i32) -> Pixel {
        if self.width == 0 || self.height == 0 {
            return [0; 4];
        }
        self.get(
            x.clamp(0, self.width as i32 - 1) as u32,
            y.clamp(0, self.height as i32 - 1) as u32,
        )
    }

    /// From the usual image, with the rows from the top. The colors under transparent pixels stay.
    pub fn from_rgba(image: &image::RgbaImage) -> Image {
        let (width, height) = image.dimensions();
        let mut out = Image::new(width, height);
        for (x, y, pixel) in image.enumerate_pixels() {
            out.set(x, height - 1 - y, pixel.0);
        }
        out
    }

    pub fn to_rgba(&self) -> image::RgbaImage {
        let mut out = image::RgbaImage::new(self.width, self.height);
        for y in 0..self.height {
            for x in 0..self.width {
                out.put_pixel(x, self.height - 1 - y, image::Rgba(self.get(x, y)));
            }
        }
        out
    }

    pub fn decode(png: &[u8]) -> Result<Image> {
        Ok(Image::from_rgba(
            &image::load_from_memory(png)?.into_rgba8(),
        ))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Flip {
    X,
    Y,
    XY,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScaleMode {
    Nearest,
    Bilinear,
}

#[derive(Clone, Debug)]
pub enum Op {
    Replace(HashMap<Pixel, Pixel>),
    /// xmin, ymin, xmax, ymax
    Crop([i32; 4]),
    Multiply(Pixel),
    SetColor([u8; 3]),
    Scale(ScaleMode, [f32; 2]),
    Flip(Flip),
    /// screen or multiply, the images, the offset
    Blend(bool, Vec<String>, [i32; 2]),
    /// subtract or add, the images, the offset
    Mask(bool, Vec<String>, [i32; 2]),
    /// the width, the colors next to the pixels and far from them and whether only the outline stays
    Border(u32, Pixel, Pixel, bool),
    /// An operation that the engine does not know
    Skip,
}

fn hex_color(text: &str) -> Result<Pixel> {
    let digits: Vec<u8> = text
        .bytes()
        .map(|c| match c {
            b'0'..=b'9' => Ok(c - b'0'),
            b'a'..=b'f' => Ok(c - b'a' + 10),
            b'A'..=b'F' => Ok(c - b'A' + 10),
            _ => Err(anyhow!("'{text}' is not a color")),
        })
        .collect::<Result<_>>()?;
    Ok(match digits.len() {
        3 => [digits[0] * 17, digits[1] * 17, digits[2] * 17, 255],
        4 => [
            digits[0] * 17,
            digits[1] * 17,
            digits[2] * 17,
            digits[3] * 17,
        ],
        6 => [
            digits[0] << 4 | digits[1],
            digits[2] << 4 | digits[3],
            digits[4] << 4 | digits[5],
            255,
        ],
        8 => [
            digits[0] << 4 | digits[1],
            digits[2] << 4 | digits[3],
            digits[4] << 4 | digits[5],
            digits[6] << 4 | digits[7],
        ],
        _ => bail!("Improper size for hex string '{text}'"),
    })
}

fn number(bits: &[&str], at: usize, name: &str) -> Result<f32> {
    bits.get(at)
        .with_context(|| format!("{name} is missing a value"))?
        .parse::<f32>()
        .with_context(|| format!("{name} has a value that is not a number"))
}

fn parse_op(text: &str) -> Result<Op> {
    let bits: Vec<&str> = text.split(['=', ';']).filter(|b| !b.is_empty()).collect();
    let Some(&name) = bits.first() else {
        return Ok(Op::Skip);
    };
    Ok(match name {
        "replace" => {
            let mut map = HashMap::new();
            for pair in bits[1..].chunks(2) {
                // as in the game, a key without a value at the end is left out
                if let [from, to] = pair {
                    map.insert(hex_color(from)?, hex_color(to)?);
                }
            }
            Op::Replace(map)
        }
        "crop" => Op::Crop([
            number(&bits, 1, name)? as i32,
            number(&bits, 2, name)? as i32,
            number(&bits, 3, name)? as i32,
            number(&bits, 4, name)? as i32,
        ]),
        "multiply" => Op::Multiply(hex_color(bits.get(1).context("multiply needs a color")?)?),
        "setcolor" => {
            let c = hex_color(bits.get(1).context("setcolor needs a color")?)?;
            Op::SetColor([c[0], c[1], c[2]])
        }
        "scale" | "scalenearest" | "scalebilinear" | "scalebicubic" => {
            if name == "scalebicubic" {
                bail!("?scalebicubic is not supported");
            }
            let x = number(&bits, 1, name)?;
            let y = if bits.len() > 2 {
                number(&bits, 2, name)?
            } else {
                x
            };
            let mode = if name == "scalenearest" {
                ScaleMode::Nearest
            } else {
                ScaleMode::Bilinear
            };
            Op::Scale(mode, [x.max(0.0), y.max(0.0)])
        }
        "flipx" => Op::Flip(Flip::X),
        "flipy" => Op::Flip(Flip::Y),
        "flipxy" => Op::Flip(Flip::XY),
        "blendmult" | "blendscreen" | "addmask" | "submask" => {
            let images = bits
                .get(1)
                .with_context(|| format!("{name} needs an image"))?
                .split('+')
                .map(str::to_owned)
                .collect();
            let offset = [
                bits.get(2).map_or(Ok(0), |b| b.parse::<i32>())?,
                bits.get(3).map_or(Ok(0), |b| b.parse::<i32>())?,
            ];
            if name.starts_with("blend") {
                Op::Blend(name == "blendscreen", images, offset)
            } else {
                Op::Mask(name == "submask", images, offset)
            }
        }
        "border" | "outline" => {
            let pixels: u32 = bits
                .get(1)
                .context("border needs a width")?
                .parse()
                .context("border has a width that is not a number")?;
            let start = hex_color(bits.get(2).context("border needs a color")?)?;
            let end = bits.get(3).map_or(Ok(start), |b| hex_color(b))?;
            Op::Border(pixels, start, end, name == "outline")
        }
        "hueshift" | "saturation" | "brightness" | "fade" | "scanlines" => {
            bail!("?{name} is not supported yet")
        }
        _ => Op::Skip,
    })
}

/// Splits directives like `?crop=0;0;1;1?replace;a=b` into operations
pub fn parse(directives: &str) -> Result<Vec<Op>> {
    directives
        .split('?')
        .filter(|op| !op.is_empty())
        .map(parse_op)
        .collect()
}

/// Where the images that blends and masks refer to come from
pub trait ImageSource {
    fn image(&self, path: &str) -> Result<Image>;
}

impl<F: Fn(&str) -> Result<Image>> ImageSource for F {
    fn image(&self, path: &str) -> Result<Image> {
        self(path)
    }
}

// The engine is built with fast-math, which turns the division by 255 into a multiplication by its reciprocal, and
// that gives another last bit for some bytes.
fn byte_to_float(b: u8) -> f32 {
    b as f32 * (1.0 / 255.0)
}

fn float_to_byte(f: f32) -> u8 {
    (f.clamp(0.0, 1.0) * 255.0) as u8
}

fn lerp(t: f32, a: f32, b: f32) -> f32 {
    a * (1.0 - t) + b * t
}

fn lerp4(t: f32, a: [f32; 4], b: [f32; 4]) -> [f32; 4] {
    [
        lerp(t, a[0], b[0]),
        lerp(t, a[1], b[1]),
        lerp(t, a[2], b[2]),
        lerp(t, a[3], b[3]),
    ]
}

fn to_float4(p: Pixel) -> [f32; 4] {
    [p[0] as f32, p[1] as f32, p[2] as f32, p[3] as f32]
}

fn scaled_size(image: &Image, scale: [f32; 2]) -> (u32, u32) {
    let w = (image.width as f32 * scale[0]).round() as i32;
    let h = (image.height as f32 * scale[1]).round() as i32;
    (w.max(1) as u32, h.max(1) as u32)
}

fn scale_nearest(image: &Image, scale: [f32; 2]) -> Image {
    let (w, h) = scaled_size(image, scale);
    let mut out = Image::new(w, h);
    for y in 0..h {
        for x in 0..w {
            let sx = (x as f32 / scale[0]).round() as i32;
            let sy = (y as f32 / scale[1]).round() as i32;
            out.set(x, y, image.clamp(sx, sy));
        }
    }
    out
}

fn scale_bilinear(image: &Image, scale: [f32; 2]) -> Image {
    let (w, h) = scaled_size(image, scale);
    let mut out = Image::new(w, h);
    for y in 0..h {
        for x in 0..w {
            let pos = [x as f32 / scale[0], y as f32 / scale[1]];
            let ix = pos[0].floor() as i32;
            let iy = pos[1].floor() as i32;
            let fx = pos[0] - ix as f32;
            let fy = pos[1] - iy as f32;
            let p = |dx, dy| to_float4(image.clamp(ix + dx, iy + dy));
            let result = lerp4(fy, lerp4(fx, p(0, 0), p(1, 0)), lerp4(fx, p(0, 1), p(1, 1)));
            out.set(
                x,
                y,
                [
                    result[0] as u8,
                    result[1] as u8,
                    result[2] as u8,
                    result[3] as u8,
                ],
            );
        }
    }
    out
}

fn border(image: &Image, pixels: u32, start: Pixel, end: Pixel, outline_only: bool) -> Image {
    let p = pixels as i32;
    let mut out = Image::new(image.width + pixels * 2, image.height + pixels * 2);
    for y in 0..image.height {
        for x in 0..image.width {
            out.set(x + pixels, y + pixels, image.get(x, y));
        }
    }
    let (ow, oh) = (out.width as i32, out.height as i32);
    for y in 0..oh {
        for x in 0..ow {
            let pixel = out.get(x as u32, y as u32);
            if pixel[3] == 0 {
                let mut dist = i32::MAX;
                'search: for j in -p..p + 1 {
                    for i in -p..p + 1 {
                        if i + x >= p && j + y >= p && i + x < ow - p && j + y < oh - p {
                            let remote = image.get((i + x - p) as u32, (j + y - p) as u32);
                            if remote[3] != 0 {
                                dist = dist.min(i.abs() + j.abs());
                                if dist == 1 {
                                    break 'search;
                                }
                            }
                        }
                    }
                }
                if dist < i32::MAX {
                    let percent = (dist - 1) as f32 / (2.0 * pixels as f32 - 1.0);
                    let a = to_float4(start);
                    let b = to_float4(end);
                    let mixed = [
                        a[0] * (1.0 - percent) + b[0] * percent,
                        a[1] * (1.0 - percent) + b[1] * percent,
                        a[2] * (1.0 - percent) + b[2] * percent,
                        a[3] * (1.0 - percent) + b[3] * percent,
                    ];
                    out.set(
                        x as u32,
                        y as u32,
                        [
                            mixed[0] as u8,
                            mixed[1] as u8,
                            mixed[2] as u8,
                            mixed[3] as u8,
                        ],
                    );
                }
            } else if outline_only {
                out.set(x as u32, y as u32, [0; 4]);
            }
        }
    }
    out
}

fn blend(image: &mut Image, screen: bool, others: &[Image], offset: [i32; 2]) {
    for y in 0..image.height {
        for x in 0..image.width {
            let (px, py) = (x as i64 + offset[0] as i64, y as i64 + offset[1] as i64);
            let mut covered = false;
            let mut f = image.get(x, y).map(byte_to_float);
            for other in others {
                if px >= 0 && py >= 0 && px < other.width as i64 && py < other.height as i64 {
                    covered = true;
                    let b = other.get(px as u32, py as u32).map(byte_to_float);
                    for k in 0..4 {
                        f[k] = if screen {
                            1.0 - (1.0 - f[k]) * (1.0 - b[k])
                        } else {
                            f[k] * b[k]
                        };
                    }
                }
            }
            if covered {
                image.set(x, y, f.map(float_to_byte));
            }
        }
    }
}

fn mask(image: &mut Image, subtract: bool, others: &[Image], offset: [i32; 2]) {
    for y in 0..image.height {
        for x in 0..image.width {
            let (px, py) = (x as i64 + offset[0] as i64, y as i64 + offset[1] as i64);
            let mut alpha = 0u8;
            for other in others {
                if px >= 0 && py >= 0 && px < other.width as i64 && py < other.height as i64 {
                    let a = other.get(px as u32, py as u32)[3];
                    // the game starts both from 0, so removing a mask always clears the alpha
                    alpha = if subtract { alpha.min(a) } else { alpha.max(a) };
                }
            }
            let mut pixel = image.get(x, y);
            pixel[3] = pixel[3].min(alpha);
            image.set(x, y, pixel);
        }
    }
}

pub fn apply(ops: &[Op], mut image: Image, source: &dyn ImageSource) -> Result<Image> {
    for op in ops {
        match op {
            Op::Skip => {}
            Op::Replace(map) => {
                for pixel in &mut image.pixels {
                    if let Some(to) = map.get(pixel) {
                        *pixel = *to;
                    }
                }
            }
            Op::Crop([x0, y0, x1, y1]) => {
                let (w, h) = (x1 - x0, y1 - y0);
                if *x0 < 0
                    || *y0 < 0
                    || w < 0
                    || h < 0
                    || (*x0 + w) as u32 > image.width
                    || (*y0 + h) as u32 > image.height
                {
                    bail!(
                        "?crop of {x0};{y0};{x1};{y1} is outside the {}x{} image",
                        image.width,
                        image.height
                    );
                }
                let mut out = Image::new(w as u32, h as u32);
                for y in 0..h as u32 {
                    for x in 0..w as u32 {
                        out.set(x, y, image.get(x + *x0 as u32, y + *y0 as u32));
                    }
                }
                image = out;
            }
            Op::Multiply(color) => {
                for pixel in &mut image.pixels {
                    for k in 0..4 {
                        pixel[k] = ((pixel[k] as u32 * color[k] as u32) / 255) as u8;
                    }
                }
            }
            Op::SetColor(color) => {
                for pixel in &mut image.pixels {
                    pixel[..3].copy_from_slice(color);
                }
            }
            Op::Scale(ScaleMode::Nearest, scale) => image = scale_nearest(&image, *scale),
            Op::Scale(ScaleMode::Bilinear, scale) => image = scale_bilinear(&image, *scale),
            Op::Flip(flip) => {
                let (w, h) = (image.width, image.height);
                let mut out = Image::new(w, h);
                for y in 0..h {
                    for x in 0..w {
                        let sx = if matches!(flip, Flip::X | Flip::XY) {
                            w - 1 - x
                        } else {
                            x
                        };
                        let sy = if matches!(flip, Flip::Y | Flip::XY) {
                            h - 1 - y
                        } else {
                            y
                        };
                        out.set(x, y, image.get(sx, sy));
                    }
                }
                image = out;
            }
            Op::Blend(screen, paths, offset) => {
                let others = paths
                    .iter()
                    .map(|path| source.image(path))
                    .collect::<Result<Vec<_>>>()?;
                blend(&mut image, *screen, &others, *offset);
            }
            Op::Mask(subtract, paths, offset) => {
                let others = paths
                    .iter()
                    .map(|path| source.image(path))
                    .collect::<Result<Vec<_>>>()?;
                mask(&mut image, *subtract, &others, *offset);
            }
            Op::Border(pixels, start, end, outline_only) => {
                image = border(&image, *pixels, *start, *end, *outline_only);
            }
        }
    }
    Ok(image)
}

/// Splits `/asset.png?crop=0;0;1;1` into the path and the directives (which keep their `?`)
pub fn split_image(text: &str) -> (&str, &str) {
    text.split_at(text.find('?').unwrap_or(text.len()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn none(_: &str) -> Result<Image> {
        bail!("no images in this test")
    }

    fn sheet() -> Image {
        // a 3x2 image, rows from the bottom
        let mut image = Image::new(3, 2);
        for y in 0..2 {
            for x in 0..3 {
                image.set(x, y, [x as u8 * 50, y as u8 * 100, 7, 255]);
            }
        }
        image
    }

    fn run(directives: &str, image: Image) -> Image {
        apply(&parse(directives).unwrap(), image, &none).unwrap()
    }

    #[test]
    fn crops_from_the_bottom() {
        let out = run("?crop=1;0;3;1", sheet());
        assert_eq!((out.width, out.height), (2, 1));
        assert_eq!(out.get(0, 0), [50, 0, 7, 255]);
    }

    #[test]
    fn replace_is_simultaneous_and_reads_short_colors() {
        let image = run("?replace;f00=00f;00f=f00", {
            let mut i = Image::new(2, 1);
            i.set(0, 0, [255, 0, 0, 255]);
            i.set(1, 0, [0, 0, 255, 255]);
            i
        });
        assert_eq!(image.get(0, 0), [0, 0, 255, 255]);
        assert_eq!(image.get(1, 0), [255, 0, 0, 255]);
    }

    #[test]
    fn multiply_and_setcolor() {
        let out = run("?multiply=8080ff00?setcolor=123", sheet());
        assert_eq!(out.get(2, 1)[3], 0);
        assert_eq!(out.get(0, 0)[..3], [0x11, 0x22, 0x33]);
    }

    #[test]
    fn flips_and_scales() {
        let out = run("?flipx", sheet());
        assert_eq!(out.get(0, 0), sheet().get(2, 0));
        let out = run("?scalenearest=2", sheet());
        assert_eq!((out.width, out.height), (6, 4));
        assert_eq!(out.get(2, 2), sheet().get(1, 1));
        let out = run("?scale=1;0.5", sheet());
        assert_eq!((out.width, out.height), (3, 1));
    }

    #[test]
    fn a_border_surrounds_the_pixels_and_leaves_them() {
        let mut image = Image::new(1, 1);
        image.set(0, 0, [9, 9, 9, 255]);
        let out = run("?border=1;f00;00f", image);
        assert_eq!((out.width, out.height), (3, 3));
        assert_eq!(out.get(1, 1), [9, 9, 9, 255]);
        assert_eq!(out.get(0, 1), [255, 0, 0, 255]);
    }

    #[test]
    fn what_is_not_supported_is_an_error_and_unknown_is_skipped() {
        assert!(parse("?hueshift=30").is_err());
        let out = run("?bogus=1;2?crop=0;0;1;1", sheet());
        assert_eq!((out.width, out.height), (1, 1));
        assert!(apply(&parse("?crop=0;0;9;9").unwrap(), sheet(), &none).is_err());
    }
}
