//! # Emote directives
//!
//! A humanoid's emote frames are drawn from `emote.png` and then have the identity's `emoteDirectives`
//! applied to each 43x43 frame.
//!
//! The scheme comes from Advanced Hatter (https://github.com/KrashV/Starbound-AdvancedHatter, CC BY 3.0), which is itself
//! built on Silverfeelin's Hatter with the emote directive by NeetleBoy.
//!
//! A species prefix turns any of its frames into a 43x43 image of keys, where the pixel (x, y) of the frame with id `i`
//! is the color `(x, i, y, 0)`.
//! A single `?replace` then maps the keys of the sprite's opaque pixels to their colors.
//! The prefixes, in `emote/species.json`, are the ones that tool uses.
//! Each one picks the frames apart by the colors of a few pixels of the species' own emote sheet.
use crate::generator::{
    Input, Inputs, Species, create_replacer_string,
    data::{self, FrameSet},
    decode_rgba, do_generate,
};
use anyhow::{Context, Result, bail};
use image::RgbaImage;
use serde_json::Value;
use std::path::PathBuf;
use std::sync::LazyLock;

pub const FRAME_SIZE: u32 = 43;

/// The frames in the order of the sheet, reading from the top left, by what they are called in `humanoid/emote.frames`.
/// Where a frame is not in the sheet it is `None`
const SHEET: [[Option<&str>; 8]; 8] = [
    [
        None,
        Some("blabber.1"),
        Some("blabber.2"),
        None,
        Some("shout.1"),
        Some("shout.2"),
        None,
        None,
    ],
    [
        None,
        Some("happy.1"),
        Some("happy.2"),
        None,
        None,
        None,
        None,
        Some("idle.1"),
    ],
    [
        None,
        Some("sad.1"),
        Some("sad.2"),
        Some("sad.3"),
        Some("sad.4"),
        Some("sad.5"),
        None,
        None,
    ],
    [
        None,
        Some("neutral.1"),
        Some("neutral.2"),
        None,
        Some("laugh.1"),
        Some("laugh.2"),
        None,
        None,
    ],
    [
        None,
        Some("annoyed.1"),
        Some("annoyed.2"),
        None,
        None,
        None,
        None,
        None,
    ],
    [
        None,
        Some("oh.1"),
        Some("oh.2"),
        None,
        Some("oooh.1"),
        Some("oooh.2"),
        Some("oooh.3"),
        None,
    ],
    [None; 8],
    [
        None,
        Some("blink.1"),
        Some("blink.2"),
        None,
        Some("wink.1"),
        Some("wink.2"),
        Some("wink.3"),
        Some("wink.4"),
    ],
];

/// The first digit of the ids, i.e., which rows of the sheet share one?
const GROUPS: [&[usize]; 6] = [&[0, 1], &[2], &[3], &[4], &[5], &[7]];

static PREFIXES: LazyLock<Value> = LazyLock::new(|| {
    serde_json::from_str(include_str!("emote/species.json")).expect("emote/species.json")
});

fn species_name(species: Species) -> Result<&'static str> {
    Ok(match species {
        Species::Apex => "apex",
        Species::Avian => "avian",
        Species::Floran => "floran",
        Species::Glitch => "glitch",
        Species::Human => "human",
        Species::Hylotl => "hylotl",
        Species::Novakid => "novakid",
        Species::Other => bail!("there are no emote prefixes for other species"),
    })
}

/// One frame of the sheet.
pub struct Frame {
    pub name: &'static str,
    pub column: u32,
    pub row: u32,
    pub id: u8,
}

pub fn frames() -> Vec<Frame> {
    let mut frames = vec![];
    for (group, rows) in GROUPS.iter().enumerate() {
        let mut index = 0;
        for &row in *rows {
            for (column, name) in SHEET[row].iter().enumerate() {
                if let Some(name) = name {
                    index += 1;
                    frames.push(Frame {
                        name,
                        column: column as u32,
                        row: row as u32,
                        id: 0xa0 + 0x10 * group as u8 + index,
                    });
                }
            }
        }
    }
    frames
}

/// The directives that turn any emote frame of the species into the keys `(x, id, y, 0)`.
pub fn prefix(species: Species) -> Result<&'static str> {
    let name = species_name(species)?;
    PREFIXES[name]
        .as_str()
        .with_context(|| format!("no emote prefix for {name}"))
}

/// The frames that the species' prefix has an id for.
/// A species whose vanilla frames look the same in the few pixels that the prefix looks at can't tell
/// them apart.
/// Frames without an id are drawn like another one.
pub fn known_ids(species: Species) -> Result<Vec<u8>> {
    let mut ids: Vec<u8> = prefix(species)?
        .split(';')
        .filter_map(|pair| pair.split_once('=')?.1.strip_prefix("00"))
        .filter(|id| id.len() >= 4 && id.is_char_boundary(4))
        .filter_map(|id| u8::from_str_radix(&id[..2], 16).ok())
        .collect();
    ids.sort_unstable();
    ids.dedup();
    Ok(ids)
}

/// The frames that can't be set for the species, see `known_ids`.
pub fn legacy_unreachable_frames(species: Species) -> Result<Vec<Frame>> {
    let ids = known_ids(species)?;
    Ok(frames()
        .into_iter()
        .filter(|frame| !ids.contains(&frame.id))
        .collect())
}

/// The key that the frame's pixel at (x, y), counted from the top left, has.
/// Rows start at the bottom.
pub fn key(id: u8, x: u32, y: u32) -> [u8; 4] {
    [x as u8, id, (FRAME_SIZE - 1 - y) as u8, 0]
}

pub fn legacy_directives(sheet: &RgbaImage, species: Species) -> Result<String> {
    let (width, height) = sheet.dimensions();
    if height != FRAME_SIZE * 8 || width < FRAME_SIZE * 8 {
        bail!(
            "the emote sheet should be 8 rows of 43 pixels and at least 8 columns ({} x {}), this one is {width}x{height}",
            FRAME_SIZE * 9,
            FRAME_SIZE * 8
        );
    }
    let mut replacements = vec![];
    let ids = known_ids(species)?;
    for frame in frames().into_iter().filter(|frame| ids.contains(&frame.id)) {
        for y in 0..FRAME_SIZE {
            for x in 0..FRAME_SIZE {
                let pixel =
                    sheet.get_pixel(frame.column * FRAME_SIZE + x, frame.row * FRAME_SIZE + y);
                if pixel[3] != 0 {
                    replacements.push((key(frame.id, x, y), pixel.0));
                }
            }
        }
    }
    replacements.sort_unstable();
    let mut out = String::from(prefix(species)?);
    out.push_str(&create_replacer_string(replacements, false, |_| true));
    Ok(out)
}

/// The frame set of the species.
///
/// A prefix that turns its frames into 2x2 images with distinct corners and shows where the frames are in the sheet.
/// Frames that look the same as an earlier one in the vanilla sheet are not in it.
pub fn frame_set(species: Species) -> Result<&'static FrameSet> {
    Ok(match species {
        Species::Apex => &data::EMOTE_APEX,
        Species::Avian => &data::EMOTE_AVIAN,
        Species::Floran => &data::EMOTE_FLORAN,
        Species::Glitch => &data::EMOTE_GLITCH,
        Species::Human => &data::EMOTE_HUMAN,
        Species::Hylotl => &data::EMOTE_HYLOTL,
        Species::Novakid => &data::EMOTE_NOVAKID,
        Species::Other => bail!("there are no emote prefixes for other species"),
    })
}

/// The frames that the species' frame set does not have, because the vanilla sheet draws them like an earlier frame
pub fn unreachable_frames(species: Species) -> Result<Vec<Frame>> {
    let set = frame_set(species)?;
    Ok(frames()
        .into_iter()
        .filter(|frame| !set.entries.iter().any(|entry| entry.frame == frame.name))
        .collect())
}

/// The emote directives of the sheet (a png file), made with the generator
pub fn directives(png: &[u8], species: Species) -> Result<String> {
    let (width, height) = image::ImageReader::new(std::io::Cursor::new(png))
        .with_guessed_format()?
        .into_dimensions()?;
    if (width, height) != (FRAME_SIZE * 9, FRAME_SIZE * 8) {
        bail!(
            "the emote sheet should be {}x{}, this one is {width}x{height}",
            FRAME_SIZE * 9,
            FRAME_SIZE * 8
        );
    }
    let input = Input {
        filename: "emote.png".to_owned(),
        data: png.to_vec(),
        width,
        height,
    };
    let mut inputs = Inputs::new();
    inputs.insert("emote".to_string(), (input, 9));
    Ok(do_generate(&inputs, frame_set(species)?, "emote")?.directives)
}

/// `legacy` makes the directives the way Advanced Hatter does, a key for every pixel, which are several times longer
pub fn generate(
    sheet: &std::path::Path,
    species: Species,
    legacy: bool,
    output: Option<PathBuf>,
) -> Result<()> {
    let data =
        std::fs::read(sheet).with_context(|| format!("failed to read {}", sheet.display()))?;
    let (directives, left_out) = if legacy {
        let image =
            decode_rgba(&data).with_context(|| format!("failed to decode {}", sheet.display()))?;
        (
            legacy_directives(&image, species)?,
            legacy_unreachable_frames(species)?,
        )
    } else {
        (directives(&data, species)?, unreachable_frames(species)?)
    };
    for frame in left_out {
        eprintln!(
            "(the {} frame {} looks like another one to the prefix, so it is left out)",
            species_name(species)?,
            frame.name
        );
    }
    eprintln!("{} chars", directives.len());
    crate::generator::write_output(output, "EmoteDirective.txt", directives)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_follow_the_reference() {
        let frames = frames();
        assert_eq!(frames.len(), 29);
        let id = |name: &str| frames.iter().find(|f| f.name == name).unwrap().id;
        assert_eq!(id("blabber.1"), 0xa1);
        assert_eq!(id("idle.1"), 0xa7);
        assert_eq!(id("sad.5"), 0xb5);
        assert_eq!(id("oooh.2"), 0xe4);
        assert_eq!(id("wink.4"), 0xf6);
    }

    #[test]
    fn every_species_has_a_prefix() {
        for species in [
            Species::Apex,
            Species::Avian,
            Species::Floran,
            Species::Glitch,
            Species::Human,
            Species::Hylotl,
            Species::Novakid,
        ] {
            assert!(prefix(species).unwrap().starts_with("?crop;17;21;27;31"));
        }
        assert!(prefix(Species::Other).is_err());
    }
}
