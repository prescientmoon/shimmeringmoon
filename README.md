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

## How does it work

- The bot uses [poise](https://github.com/serenity-rs/poise) in order to communicate with discord
- The bot renders images using [my own custom bitmap renderer & layout system](./src/bitmap.rs)
- The bot recognises images using [my own jacket recognition algorithm](./src/arcaea/jacket.rs)
- The bot reads text using [my own OCR algorithm](./src/recognition/hyperglass.rs). The project started off by using [Tesseract](https://github.com/tesseract-ocr/tesseract), although it was unreliable, and had big issues reading fonts with a lot of kerning (like Arcaea's song font for the bigrams `74` and `24`). My implementation is much more accurate because it's much less general purpose, and uses knowledge of the font to achieve better results.

No neural-networks/machine-learning is used by this project. All image analysis is done using classical algorithms I came up with by glueing basic concepts together.

## Running locally

The bot needs the following environment variables to be set in order to run:

```
SHIMMERING_DISCORD_ID=yourtoken
SHIMMERING_DATA_DIR=shimmering/data
SHIMMERING_ASSET_DIR=shimmering/assets
SHIMMERING_CONFIG_DIR=shimmering/config
SHIMMERING_LOG_DIR=shimmering/logs
```

### Fonts

The following fonts must be present in `$SHIMMERING_ASSET_DIR/fonts`:

```
arial.ttf
exo-variable.ttf
geosans-light.ttf
kazesawa-bold.ttf
kazesawa-regular.ttf
noto-sans.ttf
saira-variable.ttf
unifont.otf
```

### Assets

Most of the assets in this repo have been drawn by me, although you need to bring in your own song jackets and place them at `$SHIMMERING_ASSET_DIR/songs`. This subdirectory must contain a subdirectory for each song in the game, with such subdirectories containing their default jacket at `base_256.jpg`. Different files can be created to override the jacket for each difficulty. For more details, check out the implementation in [./src/arcaea/jacket.rs](./src/arcaea/jacket.rs).

Additionally, you must place a custom `b30` background at `$SHIMMERING_ASSET_DIR/b30_background.jpg`.

> [!CAUTION]
> As far as I am concerned, the code in this repository does not violate the Arcaea terms of service in any way. Importing jackets that have been datamined/ripped out of the game is against the aforementioned TOS, and is highly discouraged.

### Importing charts

The charts are stored in [$SHIMMERING_CONFIG_DIR/charts.csv](./shimmering/config/charts.csv). This is a csv-version of Lumine's [Arcaea song table](https://tinyurl.com/mwd5dkfw) ([with permission](https://discord.com/channels/399106149468733441/399106149917392899/1256043659355226163)). Importing song-data from any other source (such as datamined database files) will not only be more difficult for you (all the scripts I have written are built around the aforementioned spreadsheet), but is also against the Arcaea terms of service.

To add charts that have just been added to the CSV file into the database, run [import-charts.py](./scripts/import-charts.py).

## Thanks

Many thanks go to:

- `@.luminexus` for providing the amazing [Arcaea song table](https://tinyurl.com/mwd5dkfw)
- `@siloricity` for helping with development assets
- `@black._heart_.sl` for being the first person I discussed this idea extensively with
- `@dyuan01` for discussing different scoring system ideas with me
- [George Dragomir](https://github.com/BlueGhostGH) for, at my request, writing [a new set](https://github.com/BlueGhostGH/hypertesseract) of [Tesseract](https://github.com/tesseract-ocr/tesseract) bindings for the Rust programming language. The [popular rust bindings for Tesseract](https://crates.io/crates/tesseract) are incomplete, unidiomatic, painful to use, easy to misuse, and leak copious amounts of memory. Please avoid them at all cost.
- The members of a certain small-scale Arcaea server for enduring my shimmeringmoon-related rambles :3
