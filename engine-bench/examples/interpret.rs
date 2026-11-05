//! Checks `sbgen::interpret` against the engine.
//!
//! The same directives on the same images must give the same pixels.
//! The cases are what the generators make (single sprites, the emote frame sets) and what otherr generators make
//! (Advanced Hatter's and Silverfeelin's), plus directives of every supported operation.
//!
//! Usage: interpret [more directive files, each applied to /assetmissing.png]
use anyhow::Result;
use engine_bench::engine::{Directives, Image, Pak};
use sbgen::{emote, generator::Species, interpret};

fn from_engine(image: &Image) -> interpret::Image {
    interpret::Image {
        width: image.width,
        height: image.height,
        pixels: image
            .rgba
            .chunks_exact(4)
            .map(|p| p.try_into().unwrap())
            .collect(),
    }
}

struct Checker<'a> {
    pak: &'a Pak,
    cases: usize,
    bad: usize,
}

impl Checker<'_> {
    fn check(&mut self, label: &str, base: &Image, directives: &str) -> Result<()> {
        self.cases += 1;
        let mut engine = Directives::new(directives)?;
        engine.use_images_from(self.pak);
        engine.load()?;
        let want = engine
            .apply_sequential_in_place(base)
            .map(|(w, h, p)| Image {
                width: w,
                height: h,
                rgba: p.to_vec(),
            });
        let source = |path: &str| -> Result<interpret::Image> {
            Ok(from_engine(&Image::decode(&self.pak.read(path)?)?))
        };
        let got = interpret::parse(directives)
            .and_then(|ops| interpret::apply(&ops, from_engine(base), &source));
        match (want, got) {
            (Ok(want), Ok(got)) => {
                let want = from_engine(&want);
                if want != got {
                    self.bad += 1;
                    let differing = want
                        .pixels
                        .iter()
                        .zip(&got.pixels)
                        .filter(|(a, b)| a != b)
                        .count();
                    let first = want
                        .pixels
                        .iter()
                        .zip(&got.pixels)
                        .position(|(a, b)| a != b);
                    println!(
                        "DIFFERENT {label}: {}x{} vs {}x{}, {differing} pixels, first {first:?} {:?}",
                        want.width,
                        want.height,
                        got.width,
                        got.height,
                        first.map(|i| (want.pixels[i], got.pixels[i]))
                    );
                }
            }
            (Err(_), Err(_)) => {}
            (Ok(_), Err(e)) => {
                self.bad += 1;
                println!("PORT FAILS {label}: {e:#}");
            }
            (Err(e), Ok(_)) => {
                self.bad += 1;
                println!("ENGINE FAILS {label}: {e:#}");
            }
        }
        Ok(())
    }
}

fn main() -> Result<()> {
    let pak = Pak::open_default()?;
    let mut c = Checker {
        pak: &pak,
        cases: 0,
        bad: 0,
    };
    let missing = Image::decode(&pak.read("/assetmissing.png")?)?;
    println!("/assetmissing.png is {}x{}", missing.width, missing.height);

    // the png decoding agrees with the engine's rows
    let png = pak.read("/humanoid/human/emote.png")?;
    let engine_image = from_engine(&Image::decode(&png)?);
    assert_eq!(
        interpret::Image::decode(&png)?,
        engine_image,
        "decoding differs"
    );

    // every operation on a few base images
    let bases = [
        ("assetmissing", missing.clone()),
        ("emote sheet", Image::decode(&png)?),
        (
            "body",
            Image::decode(&pak.read("/humanoid/human/malebody.png")?)?,
        ),
    ];
    let ops = [
        "?crop=0;0;1;1",
        "?crop;3;5;40;30",
        "?flipx",
        "?flipy",
        "?flipxy",
        "?multiply=8f40ffc0",
        "?multiply=0000",
        "?setcolor=f0a",
        "?replace;ffffff00=123;00000000=4567",
        "?scale=2",
        "?scale=0.5",
        "?scale=.8;.6",
        "?scale=1.37;0.61",
        "?scale=47",
        "?scale=0.3",
        "?scalenearest=3",
        "?scalenearest=0.4;1.3",
        "?scalebilinear=1.7",
        "?border=1;ccc0;0000",
        "?border=2;f00;00f",
        "?border=3;fff;0004",
        "?outline=1;f00;00f",
        "?blendmult=/objects/outpost/customsign/signplaceholder.png",
        "?blendmult=/objects/outpost/customsign/signplaceholder.png;3;-2",
        "?blendscreen=/ships/apex/apexT3blocks.png;6;20?multiply=00ffff",
        "?addmask=/ships/apex/apexT3blocks.png;2;3",
        "?submask=/ships/apex/apexT3blocks.png",
        "?crop=2;3;30;40?scale=1.3?border=2;f00;00f?flipx?multiply=ffe0?scale=0.77;0.9",
    ];
    for (name, base) in &bases {
        for op in ops {
            c.check(&format!("{name} {op}"), base, op)?;
        }
    }

    // Advanced Hatter's and Silverfeelin's way of drawing a sprite, and our own
    let hatter = "?crop;0;0;2;2?replace;fff0=fff;0000=fff?setcolor=fff?blendmult=/objects/outpost/customsign/signplaceholder.png\
        ?replace;01000101=01000100;02000101=2B000100?replace;01000201=01002B00;02000201=2B002B00?scale=43?crop=0;0;43;43\
        ?replace;000000=f00;010001=0f0;020002=00f;2a002a=fff";
    c.check("hatter", &missing, hatter)?;
    for name in [
        "frame",
        "frame_recolor_flip",
        "frame_edited",
        "sheet_recolor",
    ] {
        let path = format!(
            "/tmp/claude-1000/-home-lumi-code-game-stargen/317f22da-a5ea-49d0-a7be-035684b6f9e8/scratchpad/closest/{name}.png"
        );
        if let Ok(png) = std::fs::read(&path) {
            let (w, h) = image::ImageReader::new(std::io::Cursor::new(&png))
                .with_guessed_format()?
                .into_dimensions()?;
            let mut inputs = sbgen::generator::Inputs::new();
            inputs.insert(
                "image".into(),
                (
                    sbgen::generator::Input {
                        filename: format!("{name}.png"),
                        data: png,
                        width: w,
                        height: h,
                    },
                    0,
                ),
            );
            if let Ok(result) =
                sbgen::generator::do_generate(&inputs, &sbgen::generator::data::SINGLE, "image")
            {
                c.check(&format!("single {name}"), &missing, &result.directives)?;
            }
        }
    }

    // the emote frame sets on the vanilla frames
    for (name, species) in [
        ("human", Species::Human),
        ("apex", Species::Apex),
        ("glitch", Species::Glitch),
        ("avian", Species::Avian),
        ("novakid", Species::Novakid),
    ] {
        let sheet = pak.read(&format!("/humanoid/{name}/emote.png"))?;
        let directives = emote::directives(&sheet, species)?;
        let image = Image::decode(&sheet)?;
        for frame in emote::frames() {
            c.check(
                &format!("emote {name} {}", frame.name),
                &image.crop(frame.column * 43, (7 - frame.row) * 43, 43, 43),
                &directives,
            )?;
        }
        c.check(
            &format!("emote legacy prefix {name}"),
            &image.crop(43, 0, 43, 43),
            emote::prefix(species)?,
        )?;
    }

    for file in std::env::args().skip(1) {
        c.check(&file, &missing, std::fs::read_to_string(&file)?.trim())?;
    }
    // PAIR=1: the first file is old directives and the second the converted ones, which have to draw the same
    if std::env::var_os("PAIR").is_some() {
        let files: Vec<String> = std::env::args().skip(1).collect();
        let draw = |file: &str| -> Result<Image> {
            let mut d = Directives::new(std::fs::read_to_string(file)?.trim())?;
            d.use_images_from(&pak);
            d.load()?;
            let (w, h, p) = d.apply_sequential_in_place(&missing)?;
            Ok(Image {
                width: w,
                height: h,
                rgba: p.to_vec(),
            })
        };
        let (old, new) = (draw(&files[0])?, draw(&files[1])?);
        let same = old.width == new.width
            && old.height == new.height
            && old
                .rgba
                .chunks_exact(4)
                .zip(new.rgba.chunks_exact(4))
                .all(|(a, b)| (a[3] == 0 && b[3] == 0) || a == b);
        println!(
            "old and converted directives draw {}",
            if same {
                "the same image"
            } else {
                "DIFFERENT images"
            }
        );
        if !same {
            c.bad += 1;
        }
    }
    println!("{} cases, {} differ", c.cases, c.bad);
    std::process::exit((c.bad > 0) as i32);
}
