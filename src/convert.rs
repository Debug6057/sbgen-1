//! Converts the directives that other generators made.
//! First, draws the image they make.
//! Then, generates it again under sbgen.
use crate::generator::{self, Input, Inputs, SpriteOutputType};
use crate::interpret::{self, Image};
use crate::pak::Assets;
use anyhow::{Context, Result, bail};
use std::path::{Path, PathBuf};

/// Where to report directives that come out longer
const ISSUES: &str = concat!(env!("CARGO_PKG_REPOSITORY"), "/issues");

/// The length of the new directives against the old ones, as the sentence that is printed
pub fn report(old: usize, new: usize) -> String {
    let change = (new as f64 / old.max(1) as f64 - 1.0) * 100.0;
    if new < old {
        format!("{old} chars -> {new} chars, {:.1}% shorter", change.abs())
    } else if new == old {
        format!("{old} chars -> {new} chars, the same length")
    } else {
        format!(
            "{old} chars -> {new} chars, {change:.1}% LONGER than the original\n\
             That should not happen! Please report it at {ISSUES} with the image and the directives that were converted"
        )
    }
}

/// The png of the image that the directives draw on the base image
pub fn draw(base: Image, directives: &str, assets: Option<&Assets>) -> Result<Image> {
    let source = |path: &str| -> Result<Image> {
        let assets = assets.context("the directives use another image of the game and there are no game assets to read it from")?;
        Image::decode(
            &assets
                .read(path)
                .with_context(|| format!("failed to read {path} from the game assets"))?,
        )
    };
    interpret::apply(&interpret::parse(directives)?, base, &source)
}

fn encode_png(image: &image::RgbaImage) -> Result<Vec<u8>> {
    let mut data = vec![];
    image.write_to(
        &mut std::io::Cursor::new(&mut data),
        image::ImageFormat::Png,
    )?;
    Ok(data)
}

/// Generates the directives for a drawn image and returns them with the length of the old ones
pub fn regenerate(drawn: &Image, name: &str) -> Result<String> {
    let rgba = drawn.to_rgba();
    let (width, height) = rgba.dimensions();
    let mut inputs = Inputs::new();
    let input = Input {
        filename: format!("{name}.png"),
        data: encode_png(&rgba)?,
        width,
        height,
    };
    inputs.insert("image".to_string(), (input, 0));
    Ok(generator::do_generate(&inputs, &generator::data::SINGLE, "image")?.directives)
}

/// `image` is a file or a path in the game's assets (`/assetmissing.png`) and can have the directives after it (`?...`).
/// `directives` are more directives, or `@file` to read them from a file (they get long).
pub fn convert(
    image: &str,
    directives: Option<&str>,
    packs: &[PathBuf],
    output_type: SpriteOutputType,
    output: Option<PathBuf>,
) -> Result<()> {
    let (path, attached) = interpret::split_image(image);
    let mut old = attached.to_owned();
    if let Some(more) = directives {
        match more.strip_prefix('@') {
            Some(file) => old.push_str(
                std::fs::read_to_string(file)
                    .with_context(|| format!("failed to read {file}"))?
                    .trim(),
            ),
            None => old.push_str(more),
        }
    }
    if old.trim_matches('?').is_empty() {
        bail!("there are no directives to convert");
    }

    let assets = if packs.is_empty() {
        Assets::open_default().ok()
    } else {
        Some(Assets::open(packs)?)
    };
    let base = if Path::new(path).is_file() {
        Image::decode(&std::fs::read(path).with_context(|| format!("failed to read {path}"))?)?
    } else if path.starts_with('/') {
        let assets = assets.as_ref().with_context(|| {
            format!("{path} is not a file and there are no game assets to look in")
        })?;
        Image::decode(
            &assets
                .read(path)
                .with_context(|| format!("{path} is not a file or an asset of the game"))?,
        )?
    } else {
        bail!("{path} is not a file, and an asset of the game starts with /");
    };

    let drawn = draw(base, &old, assets.as_ref())?;
    eprintln!(
        "the old directives draw a {}x{} image",
        drawn.width, drawn.height
    );
    let name = Path::new(path).file_stem().map_or_else(
        || "converted".to_owned(),
        |n| n.to_string_lossy().into_owned(),
    );
    let new = regenerate(&drawn, &name)?;
    eprintln!("{}", report(old.len(), new.len()));
    let content = generator::sprite_content(output_type, &new, None);
    generator::write_output(output, &format!("{name}.txt"), content)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgba, RgbaImage};

    #[test]
    fn the_report_says_shorter_longer_and_the_same() {
        assert_eq!(report(200, 50), "200 chars -> 50 chars, 75.0% shorter");
        assert!(report(10, 10).contains("same length"));
        let longer = report(100, 150);
        assert!(longer.contains("50.0% LONGER") && longer.contains("/issues"));
    }

    #[test]
    fn directives_draw_back_to_the_sprite_that_made_them() {
        let mut sprite = RgbaImage::new(5, 4);
        for (x, y, color) in [
            (0, 0, [200, 10, 10, 255]),
            (1, 0, [200, 10, 10, 255]),
            (4, 3, [10, 200, 10, 128]),
            (2, 1, [1, 2, 3, 255]),
        ] {
            sprite.put_pixel(x, y, Rgba(color));
        }
        let made = regenerate(&Image::from_rgba(&sprite), "test").unwrap();
        // the directives start from any image, whose first pixel they look at
        let mut base = Image::new(2, 2);
        base.pixels[0] = [9, 9, 9, 255];
        let drawn = draw(base, &made, None).unwrap().to_rgba();
        assert_eq!(drawn.dimensions(), sprite.dimensions());
        for (x, y, pixel) in sprite.enumerate_pixels() {
            let got = drawn.get_pixel(x, y);
            assert!(
                pixel[3] == 0 && got[3] == 0 || got == pixel,
                "{x},{y}: {got:?} vs {pixel:?}"
            );
        }
    }
}
