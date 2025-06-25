# Shimmeringmoon

Arcaea screenshot analyzer!

This bot analyzes your Arcaea screenshots (both of your scores, and taken in the song-select menu), extracts score data from them, and keeps track of such score data in a database. This bot is still in development. Contact `@prescientmoon` on discord if you want to help out in any way.

## Features

- song/chart info queries
- score queries (eg: listing your best score for a given chart)
- B30 (heck, even B300, if you so desire) rendering
- Multiple scoring systems to choose from (including sdvx like EX-scoring)
- Achievements (work in progress)
- Graph plotting (work in progress)
- Optional integration with private server implementations.

## How does it work

- The bot uses [poise](https://github.com/serenity-rs/poise) in order to communicate with discord
- The bot renders images using [my own custom bitmap renderer & layout system](./src/bitmap.rs)
- The bot recognises images using [my own jacket recognition algorithm](./src/arcaea/jacket.rs)
- The bot reads text using [my own OCR algorithm](./src/recognition/hyperglass.rs). The project started off by using [Tesseract](https://github.com/tesseract-ocr/tesseract), although it was unreliable, and had big issues reading fonts with a lot of kerning (like Arcaea's song font for the bigrams `74` and `24`). My implementation is much more accurate, as it is much less general purpose, and uses knowledge of the font to achieve better results.

No neural-networks/machine-learning is used by this project. All image analysis is done using classical algorithms I came up with by glueing basic concepts together.

## Running locally

> [!WARNING]
> The instructions that used to live in this file are a bit outdated. I'll one day write up to date instructions, but for now your best bet is checking out the [.nix](./nix) directory and hoping for the best. I have all the builds (and the deployment) automated, although the derivations depend on two private repos of mine containing assets and other data.

### Binaries

The project currently exposes two binaries:

1. `shimmering-discord-bot` provides (as the name suggests) a discord bot exposing the `shimmeringmoon` functionality
2. `shimmering-cli` provides (again, as the name suggests) a command line interface for administration and debugging purposes:

   - The `prepare-jackets` command prepares the provided jackets for running the bot (see the section below for more details)
   - The `analyse <...paths>` command is a command-line version of the `score magic` discord command. This is useful for debugging things like the OCR implementation, without having to transmit files over the network.

### Work in progress

These binaries are unstable at best, and broken at worst.

3. `shimmering-server` provides functionality over HTTP
4. `shimmering-discord-presence` is a client application that talks to `shimmering-server` in order to update your discord "currently playing", showing off the scores you are getting.

### Assets

Most of the assets in this repo have been drawn by me. You need to bring in your own song jackets and place them in `$SHIMMERING_PRIVATE_CONFIG_DIR/jackets`. This directory must contain a subdirectory for each song in the game, with each subdirectory containing a default jacket at `base_256.jpg`. Different files can be created to override the jacket for each difficulty. For more details, check out the implementation in [./src/arcaea/jacket.rs](./src/arcaea/jacket.rs).

Additionally, you must place a custom `b30` background at `$SHIMMERING_COMPTIME_PRIVATE_CONFIG_DIR/b30_background.jpg`. This file must be present at compile-time, and is embedded into the resulting binary.

> [!CAUTION]
> As far as I am concerned, the code in this repository does not violate the Arcaea terms of service in any way. Importing jackets that have been datamined/ripped out of the game is against the aforementioned TOS, and is highly discouraged.

After everything has been placed in the right directory, run `shimmeringmoon-cli prepare-jackets` to prepare everything. This will:

- Associate each asset with its database ID
- Build out a recognition matrix (about $30\text{K}$) for image recognition purposes. This file contains:
  - about $3$ pixels worth of information for each jacket, stored together with the respective database ID
  - a projection matrix which transforms a $8 \times 8$ downscaled vectorized version of an image (that's $192$ dimensions — $64 \text{ pixels} \times 3 \text{ channels}$) and projects it to a $10$ dimensional space (the matrix is built using [truncated singular value decomposition](https://en.wikipedia.org/wiki/Singular_value_decomposition)).

### Importing charts

> [!NOTE]
> I need to write down some up to date instructions for how to do this. For now, note note count data is stored in [./src/arcaea/notecounts.csv](./src/arcaea/notecounts.csv). A CSV version of the song/chart info can be found in the [./info](./info) directory (this data has originally been extracted from community-run B30 spreadsheets, although many changes have since been manually made by me to standardize things). As to how to generate the db... it's... complicated. I need to take the time to standardize the process some day.

## Testing

The project provides an always-growing automated test suite for its core functionality. The command logic is written in terms of a generic `MessagingContext` trait, which allows running the commands in non-discord contexts. The technique employed is called "golden testing" (also known as "snapshot testing") — the output of each test is initially saved to disk (at [test/commands](./test/commands)). On subsequent runs, the output is compared to the existing files, with the test failing on mismatches. You can provide the `SHIMMERING_TEST_REGEN=1` environment variable to override the existing output (make sure the changes are intended).

Each test saves its output in a directory. Each file tracks the contents of a single response the bot produced during testing. This file contains everything from whether the response was a reply or not, to every field of every embed, to the hash of every attachment.

The screenshots used for testing are not available in this repository. Although thousands of Arcaea screenshots are posted to the internet on a daily basis, I do not want to risk any legal trouble. You need to therefore provide your own testing screenshots. The test suite expects the following files to be present in `test/screenshots`:

| File                         | Description                                 |
| ---------------------------- | ------------------------------------------- |
| `alter_ego.jpg`              | a `9_926_250` score on `ALTER EGO [ETR]`    |
| `fracture_ray_ex.jpg`        | a `9_805_651` score on `Fracture Ray [FTR]` |
| `fracture_ray_missed_ex.jpg` | a `9_766_531` score on `Fracture Ray [FTR]` |
| `antithese_74_kerning.jpg`   | a `9_983_744` score on `Antithese [FTR]`    |
| `genocider_24_kerning.jpg`   | a `9_724_775` score on `GENOCIDER [FTR]`    |

The hashes of the output images can often depend on the jacket images the tests were run with. This means you will likely have to regenerate the output locally in order to test with your own custom jackets.

## Thanks

Many thanks go to:

- `@.luminexus` for providing the amazing [Arcaea song table](https://tinyurl.com/mwd5dkfw)
- `@siloricity` for helping with development assets
- `@black._heart_.sl` for being the first person I discussed this idea extensively with
- `@dyuan01` for discussing different scoring system ideas with me
- [George Dragomir](https://github.com/BlueGhostGH) for, at my request, writing [a new set](https://github.com/BlueGhostGH/hypertesseract) of [Tesseract](https://github.com/tesseract-ocr/tesseract) bindings for the Rust programming language. The [popular rust bindings for Tesseract](https://crates.io/crates/tesseract) are incomplete, unidiomatic, painful to use, easy to misuse, and leak copious amounts of memory. Please avoid them at all cost.
- The members of a certain small-scale Arcaea server for enduring my shimmeringmoon-related rambles :3
