//! Looks for a prefix that turns every emote frame of a species into a 2x2 image whose corners tell the frames apart.
//! This is what the generator's frame sets are made of.
//! Then, writes the frame set.
//!
//! Usage: emote_prefix <species> [restarts] [seed] [out.json]    (GAP=3 sets how far apart the colors have to be)
//!
//! The engine applies every candidate to the vanilla frames, so what is found is what the game will see.
use anyhow::Result;
use engine_bench::engine::{Directives, Image, Pak};
use sbgen::emote::frames;
use std::collections::HashMap;

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
    fn chance(&mut self, percent: u64) -> bool {
        self.below(100) < percent
    }
}

/// The shortest way to write the color
fn hex(c: [u8; 4]) -> String {
    let full = format!("{:02x}{:02x}{:02x}{:02x}", c[0], c[1], c[2], c[3]);
    let b = full.as_bytes();
    let pairs = (0..4).all(|i| b[i * 2] == b[i * 2 + 1]);
    match (pairs, c[3] == 255) {
        (true, true) => (0..3).map(|i| b[i * 2] as char).collect(),
        (true, false) => (0..4).map(|i| b[i * 2] as char).collect(),
        (false, true) => full[..6].to_string(),
        (false, false) => full,
    }
}

struct Fra {
    name: &'static str,
    col: u32,
    row: u32,
    image: Image,
}

#[derive(Clone)]
struct Program {
    fill: Option<Vec<[u8; 4]>>,
    border: Option<(u32, [u8; 4], [u8; 4])>,
    blend: Option<(bool, i32, i32)>,
    scales: Vec<(f32, Option<f32>)>,
}

struct Setup<'a> {
    pak: &'a Pak,
    reps: Vec<&'a Fra>,
    base: String,
    blank: Vec<[u8; 4]>,
    min_gap: u32,
}

impl Program {
    fn text(&self, s: &Setup) -> String {
        let mut p = s.base.clone();
        if let Some(fill) = &self.fill {
            p.push_str("?replace");
            for (b, f) in s.blank.iter().zip(fill) {
                p.push_str(&format!(";{}={}", hex(*b), hex(*f)));
            }
        }
        if let Some((w, a, b)) = self.border {
            p.push_str(&format!("?border={w};{};{}", hex(a), hex(b)));
        }
        if let Some((screen, ox, oy)) = self.blend {
            p.push_str(&format!(
                "?{}=/ships/apex/apexT3blocks.png;{ox};{oy}",
                if screen { "blendscreen" } else { "blendmult" }
            ));
        }
        for (fx, fy) in &self.scales {
            match fy {
                None => p.push_str(&format!("?scale={fx:.2}")),
                Some(fy) => p.push_str(&format!("?scale={fx:.2};{fy:.2}")),
            }
        }
        p
    }
}

struct Eval {
    collisions: usize,
    gap: u32,
    window: (usize, usize),
    size: (usize, usize),
    cost: usize,
}

fn evaluate(s: &Setup, prefix: &str) -> Option<Eval> {
    let mut d = Directives::new(prefix).ok()?;
    d.use_images_from(s.pak);
    let mut outs = vec![];
    for f in &s.reps {
        let o = d.apply(&f.image).ok()?;
        if o.width < 2 || o.height < 2 || o.width > 14 || o.height > 14 {
            return None;
        }
        outs.push(o);
    }
    if outs
        .iter()
        .any(|o| (o.width, o.height) != (outs[0].width, outs[0].height))
    {
        return None;
    }
    let (w, h) = (outs[0].width as usize, outs[0].height as usize);
    let mut best: Option<Eval> = None;
    for oy in 0..=h - 2 {
        for ox in 0..=w - 2 {
            let corners: Vec<[u8; 4]> = outs
                .iter()
                .flat_map(|o| {
                    let p = |x, y| o.pixel((ox + x) as u32, (oy + y) as u32);
                    [p(0, 0), p(1, 0), p(0, 1), p(1, 1)]
                })
                .collect();
            let (mut collisions, mut gap) = (0, u32::MAX);
            for i in 0..corners.len() {
                for j in 0..i {
                    let d = (0..4)
                        .map(|k| corners[i][k].abs_diff(corners[j][k]) as u32)
                        .max()
                        .unwrap();
                    gap = gap.min(d);
                    if d < s.min_gap {
                        collisions += 1;
                    }
                }
            }
            let cost = prefix.len()
                + if (w, h) == (2, 2) { 0 } else { 18 }
                + corners.iter().map(|c| hex(*c).len()).sum::<usize>();
            let e = Eval {
                collisions,
                gap,
                window: (ox, oy),
                size: (w, h),
                cost,
            };
            if best
                .as_ref()
                .is_none_or(|b| (e.collisions, e.cost) < (b.collisions, b.cost))
            {
                best = Some(e);
            }
        }
    }
    best
}

fn random_color(rng: &mut Rng, opaque: bool) -> [u8; 4] {
    [
        (rng.below(16) * 17) as u8,
        (rng.below(16) * 17) as u8,
        (rng.below(16) * 17) as u8,
        if opaque || rng.chance(50) {
            255
        } else {
            (rng.below(15) * 17) as u8
        },
    ]
}

fn random_scale(rng: &mut Rng) -> (f32, Option<f32>) {
    let f = |rng: &mut Rng| 0.3 + rng.below(131) as f32 * 0.01;
    let fx = f(rng);
    if rng.chance(40) {
        (fx, None)
    } else {
        (fx, Some(f(rng)))
    }
}

fn random_blend(rng: &mut Rng) -> Option<(bool, i32, i32)> {
    rng.chance(60).then(|| {
        (
            rng.chance(70),
            rng.below(60) as i32 - 30,
            rng.below(60) as i32 - 30,
        )
    })
}

fn random_program(rng: &mut Rng) -> Program {
    Program {
        fill: rng
            .chance(50)
            .then(|| (0..3).map(|_| random_color(rng, true)).collect()),
        border: rng.chance(90).then(|| {
            (
                1 + rng.below(2) as u32,
                random_color(rng, false),
                random_color(rng, false),
            )
        }),
        blend: random_blend(rng),
        scales: (0..2 + rng.below(4)).map(|_| random_scale(rng)).collect(),
    }
}

fn mutate(p: &Program, rng: &mut Rng) -> Program {
    let mut p = p.clone();
    match rng.below(8) {
        0 => {
            p.fill = rng
                .chance(60)
                .then(|| (0..3).map(|_| random_color(rng, true)).collect())
        }
        1 => {
            p.border = rng.chance(90).then(|| {
                (
                    1 + rng.below(2) as u32,
                    random_color(rng, false),
                    random_color(rng, false),
                )
            });
        }
        2 if p.scales.len() < 7 => {
            let at = rng.below(p.scales.len() as u64 + 1) as usize;
            p.scales.insert(at, random_scale(rng));
        }
        3 if p.scales.len() > 1 => {
            p.scales.remove(rng.below(p.scales.len() as u64) as usize);
        }
        7 => p.blend = random_blend(rng),
        _ => {
            let at = rng.below(p.scales.len() as u64) as usize;
            let d = |rng: &mut Rng| (rng.below(21) as f32 - 10.0) * 0.01;
            let (fx, fy) = p.scales[at];
            let fx = (fx + d(rng)).clamp(0.2, 1.8);
            p.scales[at] = match fy {
                None => (fx, None),
                Some(fy) => (fx, Some((fy + d(rng)).clamp(0.2, 1.8))),
            };
        }
    }
    p
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let species = args[1].as_str();
    let restarts: u64 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(20);
    let seed: u64 = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(1);
    let steps: u64 = std::env::var("STEPS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(1500);
    let min_gap: u32 = std::env::var("GAP")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(3);
    let pak = Pak::open_default()?;
    let sheet = Image::decode(&pak.read(&format!("/humanoid/{species}/emote.png"))?)?;
    let all: Vec<Fra> = frames()
        .into_iter()
        .map(|f| Fra {
            name: f.name,
            col: f.column,
            row: f.row,
            image: sheet.crop(f.column * 43, (7 - f.row) * 43, 43, 43),
        })
        .collect();
    // frames that look the same can only be one frame, the first of them
    let mut seen: HashMap<Vec<u8>, &str> = HashMap::new();
    let reps: Vec<&Fra> = all
        .iter()
        .filter(|f| seen.insert(f.image.rgba.clone(), f.name).is_none())
        .collect();
    let (mut x0, mut y0, mut x1, mut y1) = (43u32, 43u32, 0u32, 0u32);
    let mut blank: HashMap<[u8; 4], usize> = HashMap::new();
    for f in &reps {
        let empty = f.image.rgba.chunks_exact(4).all(|p| p[3] == 0);
        for (i, p) in f.image.rgba.chunks_exact(4).enumerate() {
            let (x, y) = (i as u32 % 43, i as u32 / 43);
            let differs = reps
                .iter()
                .any(|g| g.image.rgba[i * 4..i * 4 + 4] != reps[0].image.rgba[i * 4..i * 4 + 4]);
            if p[3] != 0 || differs {
                (x0, y0, x1, y1) = (x0.min(x), y0.min(y), x1.max(x), y1.max(y));
            }
            if p[3] == 0 && empty {
                *blank.entry(p.try_into().unwrap()).or_default() += 1;
            }
        }
    }
    let mut blank: Vec<[u8; 4]> = blank.into_keys().collect();
    blank.sort_unstable();
    println!(
        "{species}: {} distinct frames of {}, content in {x0},{y0}..{x1},{y1}, blank frames look like {blank:02x?}",
        reps.len(),
        all.len()
    );
    let setup = Setup {
        pak: &pak,
        reps,
        base: format!("?crop={x0};{y0};{};{}", x1 + 1, y1 + 1),
        blank,
        min_gap,
    };

    let mut rng = Rng(seed.wrapping_mul(0x9E3779B97F4A7C15) | 1);
    let mut winner: Option<(Program, Eval)> = None;
    for r in 0..restarts {
        let mut cur = loop {
            let p = random_program(&mut rng);
            if let Some(e) = evaluate(&setup, &p.text(&setup)) {
                break (p, e);
            }
        };
        for _ in 0..steps {
            if cur.1.collisions == 0 && cur.1.cost < 400 {
                break;
            }
            let next = mutate(&cur.0, &mut rng);
            let Some(e) = evaluate(&setup, &next.text(&setup)) else {
                continue;
            };
            if (e.collisions, e.cost) <= (cur.1.collisions, cur.1.cost) {
                cur = (next, e);
            }
        }
        println!(
            "restart {r}: {} colliding pairs, gap {}, cost {}",
            cur.1.collisions, cur.1.gap, cur.1.cost
        );
        if winner
            .as_ref()
            .is_none_or(|w| (cur.1.collisions, cur.1.cost) < (w.1.collisions, w.1.cost))
        {
            winner = Some(cur);
        }
        if winner.as_ref().is_some_and(|w| w.1.collisions == 0) && r >= 3 {
            break;
        }
    }
    let (program, eval) = winner.unwrap();
    if eval.collisions > 0 {
        {
            let mut p = program.text(&setup);
            if eval.size != (2, 2) {
                p.push_str(&format!(
                    "?crop={};{};{};{}",
                    eval.window.0,
                    eval.window.1,
                    eval.window.0 + 2,
                    eval.window.1 + 2
                ));
            }
            let mut d = Directives::new(&p)?;
            d.use_images_from(&pak);
            let mut cs: Vec<(String, [u8; 4])> = vec![];
            for f in &setup.reps {
                let o = d.apply(&f.image)?;
                for (n, (x, y)) in
                    ["ll", "lr", "ul", "ur"]
                        .iter()
                        .zip([(0, 0), (1, 0), (0, 1), (1, 1)])
                {
                    cs.push((format!("{}.{n}", f.name), o.pixel(x, y)));
                }
            }
            for i in 0..cs.len() {
                for j in 0..i {
                    let dd = (0..4)
                        .map(|k| cs[i].1[k].abs_diff(cs[j].1[k]))
                        .max()
                        .unwrap();
                    if (dd as u32) < min_gap {
                        println!(
                            "  collide: {} {:02x?} vs {} {:02x?}",
                            cs[i].0, cs[i].1, cs[j].0, cs[j].1
                        );
                    }
                }
            }
        }
        println!(
            "no prefix found, the best has {} colliding pairs: {}",
            eval.collisions,
            program.text(&setup)
        );
        std::process::exit(1);
    }
    let mut prefix = program.text(&setup);
    if eval.size != (2, 2) {
        prefix.push_str(&format!(
            "?crop={};{};{};{}",
            eval.window.0,
            eval.window.1,
            eval.window.0 + 2,
            eval.window.1 + 2
        ));
    }
    let mut d = Directives::new(&prefix)?;
    d.use_images_from(&pak);
    let mut entries = vec![];
    for f in &setup.reps {
        let o = d.apply(&f.image)?;
        assert_eq!((o.width, o.height), (2, 2));
        entries.push(serde_json::json!({
            "frame": f.name, "image": "emote", "pos": [f.col, f.row],
            "lower_left": o.pixel(0, 0), "lower_right": o.pixel(1, 0),
            "upper_left": o.pixel(0, 1), "upper_right": o.pixel(1, 1),
        }));
    }
    println!(
        "prefix ({} chars, gap {}): {prefix}",
        prefix.len(),
        eval.gap
    );
    if let Some(path) = args.get(4) {
        std::fs::write(
            path,
            serde_json::to_string(&serde_json::json!({ "prefix": prefix, "entries": entries }))?,
        )?;
        println!("wrote {path}");
    }
    Ok(())
}
