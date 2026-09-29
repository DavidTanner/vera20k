<img src="docs/images/new-conscirpt-hero-image.png" alt="VERA20k hero image" width="100%">

# VERA20k

VERA20k is a rewrite of the Red Alert 2: Yuri's Revenge engine (`gamemd.exe`) in Rust. It ports
the original game's behavior one mechanic at a time, and aims to scale up to 30 players and
20,000 units. Unlike [OpenRA](https://www.openra.net), which remakes the classic games on its
own engine, it tries to reproduce Yuri's Revenge itself.

You'll need your own copy of the game. EA sells it in *Command & Conquer The Ultimate
Collection*, on [Steam](https://store.steampowered.com/bundle/39394/) and on
[EA's site](https://www.ea.com/games/command-and-conquer/command-and-conquer-the-ultimate-collection/buy/pc).

<img src="docs/images/vera20k-screenshots.png" alt="VERA20k skirmish setup screen and in-game view" width="100%">

## Status

Pre-alpha (September 2026). You can play a local skirmish on the retail maps on Windows, against
a placeholder AI. A lot of the game works already: retail and random maps, the original menus
and sidebar, base building and power, the tech tree, the miners, infantry, vehicle, naval and
base-defense combat, garrisons, transports, engineers, cloaking, some support powers, and save
and load.

Aircraft attack runs, mind control, crates, death effects and some bridge types are partly
done. Not there yet: multiplayer, the original AI, the campaign and most map triggers, the
movies, and several special weapons and superweapons, such as the Chrono Legionnaire, Prism
chaining, the Nuke and the Chronosphere. The 30-player, 20,000-unit scale hasn't been
demonstrated yet.

## Running it

You need Rust 1.88 or newer, a GPU with Vulkan, DirectX 12 or Metal, and the game installed.
It's been played on Windows, Linux and macOS, and CI builds and tests all three.

```sh
git clone https://github.com/YuriPlanet/vera20k.git
cd vera20k
cp config.toml.example config.toml   # then set ra2_dir to your game folder
cargo run --release --bin vera20k    # always --release: debug builds are too slow to play
```

The tests don't need the game: `cargo test -p vera20k --lib`. See
[CONTRIBUTING.md](CONTRIBUTING.md#set-up) for more setup details, including where VERA20k looks
for `config.toml`.

## How it's built

The original `gamemd.exe` is the reference. Newer gameplay code names the original function it
was ported from, and [native harnesses](tools/native_oracle.md) run the original code to check
the Rust results. Most of the code is written by AI coding agents that I direct, following the
rules in [AGENTS.md](AGENTS.md).

## Contributing

Help is welcome, and you don't need reverse-engineering experience. Playing VERA20k next to the
original and reporting differences helps a lot, and so does trying it on Linux or macOS. Start
with [CONTRIBUTING.md](CONTRIBUTING.md) or the
[good first issues](https://github.com/YuriPlanet/vera20k/labels/good%20first%20issue), or say
hi on [Discord](https://discord.gg/kmjRUn5m5F). For more depth there's the
[architecture overview](https://yuriplanet.github.io/vera20k/), the
[native oracle](tools/native_oracle.md) and the [research notes](docs/research/README.md).

## Credits and legal

Thanks to OpenRA, XCC Mixer, the ModEnc wiki, Project Perfect Mod, EA's GPL source release of
Command & Conquer and Red Alert, World-Altering Editor, Final Alert, YRpp, Ares, Phobos and
many others.

Licensed under the [GPLv3](LICENSE-GPL). This repository contains no game files. Command &
Conquer and Red Alert are trademarks of Electronic Arts Inc., and the screenshots show game art
owned by Electronic Arts. VERA20k is not affiliated with or endorsed by Electronic Arts.
