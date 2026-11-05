//! Applies the emote directives that `sbgen emote` makes to the vanilla emote frames of the species with the real engine.,
//! Then, compares them to the frames of the sheet they were made from.
//! Usage: emote <sheet.png> <species> [dir for vanilla humanoid/<species>/emote.png, default: the asset pack]
use anyhow::Result;
use engine_bench::engine::{Directives, Image, Pak};
use sbgen::{emote, generator::Species};

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let species = match args[2].as_str() {
        "apex" => Species::Apex,
        "avian" => Species::Avian,
        "floran" => Species::Floran,
        "glitch" => Species::Glitch,
        "human" => Species::Human,
        "hylotl" => Species::Hylotl,
        "novakid" => Species::Novakid,
        other => anyhow::bail!("unknown species {other}"),
    };
    let pak = Pak::open_default()?;
    let path = format!("/humanoid/{}/emote.png", args[2]);
    let vanilla = match args.get(3) {
        Some(dir) => std::fs::read(format!("{dir}{path}"))?,
        None => pak.read(&path)?,
    };
    let vanilla = Image::decode(&vanilla)?;
    let custom = Image::decode(&std::fs::read(&args[1])?)?;
    let png = std::fs::read(&args[1])?;

    // EMOTE_DIRECTIVES=<file> to check another tool's directives.
    // Example: Advanced Hatter's instead of sbgen's.
    let directives = match std::env::var_os("EMOTE_DIRECTIVES") {
        Some(file) => std::fs::read_to_string(file)?.trim().to_string(),
        None => emote::directives(&png, species)?,
    };
    println!("{} chars of directives", directives.len());
    let mut directives = Directives::new(&directives)?;
    directives.use_images_from(&pak);
    directives.load()?;
    let mut total = 0;
    let skipped: Vec<_> = emote::unreachable_frames(species)?
        .into_iter()
        .map(|f| f.name)
        .collect();
    for frame in emote::frames()
        .into_iter()
        .filter(|f| !skipped.contains(&f.name))
    {
        // the engine's rows start at the bottom of the sheet
        let (x, y) = (frame.column * 43, (7 - frame.row) * 43);
        let out = directives.apply(&vanilla.crop(x, y, 43, 43))?;
        let want = custom.crop(x, y, 43, 43);
        let bad = out
            .rgba
            .chunks_exact(4)
            .zip(want.rgba.chunks_exact(4))
            .filter(|(got, want)| !(want[3] == 0 && got[3] == 0) && got != want)
            .count();
        if bad > 0 && std::env::var_os("SHOW").is_some() {
            for (i, (got, want)) in out
                .rgba
                .chunks_exact(4)
                .zip(want.rgba.chunks_exact(4))
                .enumerate()
            {
                if !(want[3] == 0 && got[3] == 0) && got != want {
                    println!(
                        "  pixel {},{} (engine rows): got {got:02x?} want {want:02x?}",
                        i % 43,
                        i / 43
                    );
                }
            }
        }
        total += bad;
        println!(
            "{:10} id {:02x}: {bad} of 1849 pixels differ",
            frame.name, frame.id
        );
    }
    println!("{total} pixels differ in all (left out: {skipped:?})");
    std::process::exit((total > 0) as i32);
}
