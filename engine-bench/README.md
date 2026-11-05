# engine-bench

Applies the directives that sbgen generates with the OpenStarbound engine to check that they draw what they should and to measure what they cost the engine.
`shim/shim.cpp` is a small C interface over the engine's own `DirectivesGroup`, `Image` and `PackedAssetSource`, linked from an existing OpenStarbound build.

## Setup

1. Build OpenStarbound. The objects of `star_core` and the others must exist in its `build/`.
2. `./setup.sh [path to OpenStarbound]` collects the compile flags and the link line into `engine/` (git ignored).
3. Add game's assets at `~/.local/share/OpenStarbound/assets/packed.pak` (or `STARBOUND_PAK`) and OpenStarbound's own assets in `~/code/game/OpenStarbound/assets/opensb` (or `OPENSB_ASSETS`), which are laid over the pack like the game does.
4. Inside the dev shell, run: `nix develop .. -c cargo run --release --example verify`.

`ENGINE_DIR=<dir with cxxflags.txt and link.txt>` at build time links another engine to compare engines.

## Tools

- `verify` applies each case's directives to every vanilla frame. Then, it compares with the sheet they were made from.
- `frames <case>` checks how many pixels are wrong in each frame.
- `inspect <case> <frame>` applies a map of one frame, `.` transparent, `#` same, `X` different, `e` missing, `g` extra.
- `steps <case> <frame>...` checks the size and visible pixels of a frame after each operation.
- `dump <case>` names the operations, one per line.
- `distinct <case>`, the strongest check, applies a custom sheet in which every cell is different (the vanilla silhouettes, colored by cell and position). This is done such that a frame cannot come out right by drawing another cell that looks the same. It also says which cell each frame draws.
- `orientation <png>` checks whether a png decodes the same in the engine and in the `image` crate (which it does not, see below!).
- `emote_prefix <species> [restarts] [seed] [out.json]` searches a prefix for the emote frames of a species and writes the frame set for `src/generator/framesets/emote_<species>.json`.
- `emote <sheet.png> <species> [vanilla dir]` applies what `sbgen emote` made to the vanilla emote frames of the species with the engine and says how many pixels differ from the sheet.
- `closest <sprite.png> "<asset path><directives>"` applies what `sbgen sprite --closest` made with the engine and says how many pixels differ from the sprite.
- `closest.sh <sprite.png> [sbgen options]`, on a sprite, checks the length of the `--closest` directives and of the ones built from nothing, whether the engine draws the sprite from each and the instructions of applying them.
- `cargo bench` runs criterion with `parse`, `apply_frame`, `apply_frame_in_order` and `apply_sheet`.
- `count.sh <work>` counts the instructions and cycles per parse, frame and sheet from `perf stat`.
- `ops.sh <work> <case>` counts the instructions each operation costs.

Times can be noisy… Run several times and make sure the machine is mostly idle, if timings are necessary.
Or, since the instruction and cycle counts from `count.sh` and `ops.sh` are not, use those to compare.
`CRITERION_HOME` is shared between runs if criterion's own comparison is wanted.

## Rows Start at the Bottom in the Engine

The engine's images start at the lower left, the `image` crate's (and so the generator's) at the top left and the frame grid of a sheet is counted from the top.
`Case::frames` takes the cell at row `rows - 1 - y` of an engine image for the generator's row `y`.
Getting this wrong swaps the rows of a sheet in pairs (0 and 5, 1 and 4, 2 and 3 of the body), so frames come out as their neighbours or blank, which can look like a bug in the generator. It is not one!!

## Changing the Engine

For work on the engine's drawable code, without building the whole engine for every change:

1. `golden > golden-before.txt` with the engine as it is. A hash of what it makes of every frame after every operation and of random images through the operations worth testing, both the way drawables are drawn and one operation after the other (about 10 000 lines).
2. Edit the core files of the worktree.
3. `GOLDEN=golden-before.txt ./try.sh <the core files that changed>` compiles just those with the engine's own flags, links them over the engine's objects (`overlay.sh`), checks that the golden output is identical and counts what a frame and a sheet cost.

Remember that directive cost is not additive.
The engine reorders, combines and caches operations, so what an operation costs when applied by itself (`ops.sh`) only points at candidates.
Judge a change by `count.sh` (the engine's own path, over whole sheets) and by the golden output.
