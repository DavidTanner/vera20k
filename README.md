<img src="docs/images/new-conscirpt-hero-image.png" alt="VERA20k hero image" width="100%">

# VERA20k

VERA20k is a rewrite of the Red Alert 2: Yuri's Revenge engine (`gamemd.exe`) in Rust. The goal
is a game that plays like the original but scales far beyond it, up to 30 players and 20,000
units.

It runs on your own copy of the game, and the original behavior gets ported over one mechanic
at a time. If you know [OpenRA](https://www.openra.net), this is a different kind of project:
OpenRA remakes the classic games on its own engine, while VERA20k tries to reproduce Yuri's
Revenge itself.

You'll need the original game, since there are no game files in here. EA still sells Red Alert
2 and Yuri's Revenge as part of *Command & Conquer The Ultimate Collection*, on
[Steam](https://store.steampowered.com/bundle/39394/) and on
[EA's site](https://www.ea.com/games/command-and-conquer/command-and-conquer-the-ultimate-collection/buy/pc).

<img src="docs/images/vera20k-screenshots.png" alt="VERA20k skirmish setup screen and in-game view" width="100%">

## Status

Pre-alpha, as of September 2026. You can play a local skirmish on the retail maps on Windows,
against a placeholder AI.

A lot of the game already works: retail and random maps, the original menus and sidebar,
building, power, the tech tree, selling and repair, War, Chrono and Slave Miners, infantry,
vehicle, naval and base-defense combat, attack dogs and Terror Drones, garrisons, transports,
engineers, cloaking, Lightning Storm, Iron Curtain and some of the other support powers, and
saving and loading.

Partly working: aircraft attack runs and Carrier Hornets, mind control, crate pickups, death
effects (Aegis sinking works, the others don't yet) and bridges (ordinary wood and concrete
bridges take damage and get fixed through the repair hut, the other variants aren't done).

Not there yet: multiplayer (the lockstep code is there, the networking isn't), the original AI,
the campaign and most map triggers, and the movies and credits. A bunch of weapons and powers
are missing too: the Chrono Legionnaire, Crazy Ivan and Magnetron among others, Gattling
spin-up, Prism chaining and Tesla charging, and the Nuke, Chronosphere, Psychic Dominator and
Spy Plane. The 30-player, 20,000-unit scale hasn't been demonstrated yet either.

## Running it

You need Rust 1.88 or newer, a GPU with Vulkan, DirectX 12 or Metal, and the game installed.
It's been played on Windows, Linux and macOS, and CI builds and tests all three.

```sh
git clone https://github.com/YuriPlanet/vera20k.git
cd vera20k
cp config.toml.example config.toml   # then set ra2_dir to your game folder
cargo run --release --bin vera20k    # always --release: debug builds are too slow to play
```

You set up matches in the original game menus. If the game files or menus fail to load, it
tells you on startup and shows which folder it searched.

It looks for `config.toml` in the folder you launch it from first, then next to the
executable. A relative `ra2_dir` is relative to the config file, and with no config at all it
looks for the game files next to the executable. For a macOS `.app`, put the config next to the
binary in `Contents/MacOS/`, so it still works when you open the app from Finder.

The tests don't need the game: `cargo test -p vera20k --lib`. There's more about setting up in
[CONTRIBUTING.md](CONTRIBUTING.md#set-up).

## How it's built

The original `gamemd.exe` is the reference. Newer gameplay code names the original function it
was ported from, and [native harnesses](tools/native_oracle.md) run the original code so the
Rust results can be checked against it. Most of the code is written by AI coding agents that I
direct, following the rules in [AGENTS.md](AGENTS.md).

## Contributing

Help is very welcome, and you don't need any reverse-engineering experience. Playing VERA20k
next to the original and reporting differences helps a lot, and so does trying it on Linux or
macOS. There are also some
[good first issues](https://github.com/YuriPlanet/vera20k/labels/good%20first%20issue) if you
want to write code. Start with [CONTRIBUTING.md](CONTRIBUTING.md), or come say hi on
[Discord](https://discord.gg/kmjRUn5m5F).

If you want to dig deeper, there's an
[architecture overview](https://yuriplanet.github.io/vera20k/), a page on the
[native oracle](tools/native_oracle.md) and the [research notes](docs/research/README.md).

## Credits and legal

Thanks to OpenRA, XCC Mixer, the ModEnc wiki, Project Perfect Mod, EA's GPL source release of
Command & Conquer and Red Alert, World-Altering Editor, Final Alert, YRpp, Ares, Phobos and
many others.

VERA20k is licensed under the [GPLv3](LICENSE-GPL). This repository contains no game files.
Command & Conquer and Red Alert are trademarks of Electronic Arts Inc., and the screenshots show
game art owned by Electronic Arts. VERA20k is not affiliated with or endorsed by Electronic
Arts.
