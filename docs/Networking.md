# Networking: CnCNet-style UDP multiplayer

Roadmap for multiplayer in VERA20k. It describes the CnCNet model, the pieces the
repository already has, the target ownership inside `src/`, and the implementation
chains in dependency order so they can be built and merged one at a time.

Status: design. Nothing below is implemented unless the "Current state" section says so.

## Goal and non-goals

Goals:

- Several VERA20k clients play one deterministic lockstep match over UDP.
- On a local network, clients find each other from inside the app and play without
  any external program, internet service or configuration file. This is what gamemd's
  Network button does over IPX, and it is the native experience VERA20k reproduces
  over UDP.
- Over the internet, play first through the CnCNet client and its tunnels, later
  from VERA20k's own lobby.

Non-goals:

- Playing against gamemd. Lockstep needs bit-identical simulation. VERA20k uses
  `SimFixed` where native uses x87, so a mixed session desyncs. CnCNet provides
  matchmaking and NAT traversal, not compatibility.
- IPX. gamemd's Network button opens an `AF_IPX` socket (`0x007B10C0`). Modern
  systems have no IPX, so VERA20k replaces it with UDP behind the same role. This is
  a recorded, necessary divergence from native.
- Reproducing gamemd's wire format. Only VERA20k clients talk to each other, so the
  packet layout is VERA20k's own. The native event semantics (opcodes, send policies,
  frame stamps, checksum history) are kept, because they decide gameplay behavior.

## How CnCNet works

CnCNet is three separate GPL-3 programs:

| Piece | What it does | Relevance to VERA20k |
|---|---|---|
| Client (`xna-cncnet-client`, C#) | IRC lobby, game negotiation, writes `spawn.ini`, launches the game, relays game UDP through tunnels | Launcher for the initial port |
| Tunnel server (`cncnet-server`, C#) | UDP relay for NAT traversal, announces itself to an HTTP master list | Transport VERA20k can talk to directly later |
| YR spawner (`yrpp-spawner`, injected DLL) | Makes gamemd start a match from `spawn.ini` instead of its menus | Not needed: VERA20k reads `spawn.ini` itself |

### Tunnel protocol, version 3

From `CnCNet/Net/Tunnel/TunnelV3.cs` in the tunnel server:

- UDP, default port 50001. Datagrams shorter than 8 bytes are dropped; receive buffer 1024 bytes.
- Header: `u32 sender_id` at offset 0, `u32 receiver_id` at offset 4, native byte
  order on the server (little-endian in practice). Payload follows unchanged.
- Ping: sender 0, receiver 0, exactly 50 bytes. The server echoes the first 12 bytes.
- Sender 0 with receiver `u32::MAX` is a maintenance command; any other sender-0 packet is dropped.
- A sender id is bound to the first endpoint that uses it. Packets with equal
  sender and receiver ids are dropped. Default limit 8 ids per IP, 200 clients.
- Master list: the server sends `GET <master>?version=3&name=..&port=..&clients=..&maxclients=..&masterpw=..&maintenance=..`
  every 60 seconds. Clients read `http://cncnet.org/master-list`, one tunnel per
  line, semicolon-separated: `address:port;country;countrycode;name;password;clients;maxclients;status;lat;lon;version;distance`.

### How the client launches a game (v3 mode)

From `GameLobbyBase.cs` and `CnCNetGameLobby.cs` in the client:

- The client reserves a local UDP port and writes `[Tunnel] Ip=127.0.0.1 Port=<reserved>`
  into `spawn.ini`. The game sends everything to loopback; the client's bridge
  forwards per player through the tunnels. The game never needs NAT traversal.
- `[Settings]`: `Name`, `Scenario`, `UIGameMode`, `UIMapName`, `MapID` (official maps only),
  `PlayerCount`, `Side`, `IsSpectator`, `Color`, `CustomLoadScreen`, `AIPlayers`, `Seed`,
  `MapSHA1`, `GameID`, `Host`, `Port`, plus `CoachMode`, `AutoSurrender` and any
  `[ForcedSpawnIniOptions]` keys from the client's `GameOptions.ini`.
- `[Other1]`..`[OtherN]`: one per remote human. `Name`, `Side`, `IsSpectator`, `Color`, `Ip`, `Port`.
- `[HouseHandicaps]`, `[HouseCountries]`, `[HouseColors]`, `[SpawnLocations]`, `[IsSpectator]`:
  keyed `MultiN`. AI players appear only here and in `AIPlayers`, never in `[OtherN]`.
- The host broadcasts `STARTV3 <GameID>;<id>;<name>;<tunnelIp>:<tunnelPort>;...` to the
  lobby; each client's bridge resolves the per-player route from it.
- The client then starts the configured executable with the spawner flags
  (`-SPAWN` for gamemd) and waits for it to exit.

Facts to confirm from client source before implementing stage 5 (not verified here):
how the loopback bridge distinguishes remote players (one local port per player, or a
single port with the game's own addressing), what `GetIPAddressForPlayer` returns in the
CnCNet lobby, and which statistics or result packets the client expects after the match.
The authoritative files are `V3GameTunnelHandler.cs`, `TunnelHandler.cs` and
`GameProcessLogic.cs`.

## Current state in the repository

| Area | State |
|---|---|
| `src/sim/event_queue.rs` | Ported native command staging and dispatch (moved from `net::lockstep` in stage 2): OutList/DoList, send policies, FrameInfo (opcode `0x1c`), checksum history, single-player and network transfer. No production caller yet; stage 2 makes it the only command path. |
| `src/sim/event_queue_convergence_tests.rs` | Convergence tests over the queue, including the stage 0 two-instance test: live semantic commands against the same orders as native records (Move, Area Guard, Scatter), with sync checksums compared every frame and an injected desync caught. |
| `src/sim/multiplayer_checksum.rs` | Stage 0: per-frame sync checksum ported from `0x0064DAB0`, computed in the frame tail before command dispatch only on the multiplayer `Queue_AI` route (`CommandQueueRoute`, which the app writes from `MatchSession` before each frame). `MatchSession` records it into the `MultiplayerChecksumHistory` ring, with the native frame-0 ring reset. Offline frames never compute it. Its parity is bounded by VERA's object registries (open display-layer writers in `display_registry.rs`). |
| `MatchSession` (`src/app/match_runtime/session.rs`) | Stage 1 done: the live session owner (mode, participant list, local participant, host flag). The launch path writes Campaign or Skirmish and the saved-game restore follows the persisted classification; the modal pump, elapsed clock and command bar read it. No writer produces `Lan` or `Wol` yet. |
| `src/app/frontend/skirmish_session.rs` | Reads the local multiplayer preferences (handle, game mode) that native refreshes before the Network dialog. |
| `src/skirmish_launch.rs` | Pre-fill plan already models several human nodes plus AI slots. |
| Main menu Network button | `bounce_network_to_main_menu` in `src/app/shell_main_menu.rs` reproduces native's silent return when the IPX socket fails. |
| Sockets, protocol, lobby | None. `src/net/mod.rs` reserves `protocol`. |

## Target ownership

`sim/` stays transport-agnostic. `net/` depends on `sim/` only. App code orchestrates.

| Responsibility | Owner |
|---|---|
| Command staging, frame stamps, dispatch order, FrameInfo, checksum history | `sim::event_queue` (exists; owned by `Simulation` from stage 2) |
| Wire framing, peer table, send and receive, tunnel header | `net::protocol` (new) |
| LAN discovery: announce, listen, game list | `net::discovery` (new, stage 7) |
| Tunnel discovery: master list, ping, selection | `net::tunnel` (new, stage 8) |
| Session mode, player list, local player id, host flag | `app::match_runtime::session::MatchSession` (exists) |
| `spawn.ini` parsing into a launch descriptor | `app/frontend` launch path, reusing `skirmish_launch` types |
| Per-frame network service in the game loop | `app/match_runtime/sim_tick` (stage 6) |
| Lobby UI | `ui/` on top of the skirmish shell (stages 7 and 9) |

Rules that apply throughout: one owner per state, no competing command paths, every
RNG draw and timer in native frame order, presentation never reads the socket.

The queue lives in `sim/`, not `net/`. Native runs `Queue_AI` (`0x00647260`) and its
`Execute_DoList` (`0x0064C380`) inside Main_Tick's tail, so the simulation frame is the
caller, and `sim/` must not depend on `net/`. `net/` moves records between the queue
and the wire.

## Implementation chains, in dependency order

Each stage is one PR. A stage is done when its "Done when" holds and the usual lib
suite and Clippy pass. Stages 1 to 6 produce the initial game port: a VERA20k that the
CnCNet client can launch and play. Stage 7 adds native LAN play from the app. Stages 8
and 9 move internet play into the app.

Stages 3, 4 and 6 are shared by LAN and internet play and must not assume loopback:
the transport binds a LAN-reachable address, and the peer table holds real endpoints.

### Stage 0: determinism groundwork (in-process guard done; cross-platform open)

Scope: hook `compute_retail_multiplayer_checksum` into `MultiplayerChecksumHistory`
per frame, and extend the convergence tests to cover the command types the live game
issues. Add a headless two-instance test that runs a scenario for N frames and compares
checksums every frame.

Done when: the two-instance test passes on macOS, Linux and Windows CI with identical
checksums per frame.

Delivered: the synthetic two-instance test runs in the lib suite without retail assets,
and the ignored retail headless test (`RA2_DIR`) compares per-frame checksums on a real
map. Both compare instances inside one process, so they catch in-process nondeterminism
and command-path divergence, not platform differences.

Residuals:
- Cross-platform equality is not yet shown. Each CI platform runs the test alone and no
  per-frame checksum sequence is pinned or compared across platforms.
- `Queue_AI`'s playback branch (`0x00A8D5F8 & 2`) also computes the checksum
  (`0x0064731C`); VERA has no production playback route.
- The ring reset driven by `[0x00A8B8B8]` belongs to the stage 6 timing service.

Why first: every later stage debugs against this. A desync found after the transport
exists is far harder to attribute.

### Stage 1: live session mode (done)

Scope: replace the hard-coded `current_session_mode` with a value the launch path
writes. Introduce the session owner holding mode, player list, local player index and
host flag. Skirmish and campaign keep writing their modes.

Done when: the modal pump, elapsed clock and any other `is_network` consumer read the
live value, and offline play is unchanged.

### Stage 2: lockstep queue in production, offline path (in progress)

Scope: route live command issue through `SynchronizedCommandQueue`, using
`transfer_single_player` and `dispatch_due_offline`. Delete the direct command path it
replaces. Keep RNG and same-frame ordering identical; the convergence tests and the
stage 0 checksum test guard this.

Done when: skirmish and campaign play through the queue with no behavior change, and
the old path is gone.

Why before transport: the queue's frame-stamp and ahead-window semantics are native
behavior the game must already obey before a second client exists.

Starting point. The live path is `Simulation::pending_commands`: semantic
`CommandEnvelope`s that `Simulation::apply_due_commands` dispatches in the frame tail
in house order, with MegaMission runs staged after each house's other orders. That is
a second implementation of `Execute_DoList` beside the queue's. Only six command kinds
had native records (Move, Move to object, Area Guard, Scatter, Exit, wall sale).

Design:

- The queue moves into `sim::event_queue` and `Simulation` owns it in place of
  `pending_commands` (see "Target ownership"). It is saved with the simulation.
- Issue encodes each command into its native record and appends it to the OutList,
  stamped with the current native frame. The 128-record OutList limit applies.
- The frame tail runs the single-player `Queue_AI` body (`0x006474DC`): transfer the
  OutList, then `Execute_DoList`. The execute callback decodes each due record into the
  semantic command the existing handlers run; the MegaMission hook runs the group
  spread (`0x0064CDA0`) on the staged records. `apply_due_commands`,
  `take_due_commands` and the app-side due-command handoff are deleted.
- Dispatch scans only houses with `IsHuman` or `PlayerControl` (`0x0064C4FB`).
- Events the simulation raises itself (the strip's ABANDON and mobile PLACE) are
  issued into the OutList like player input, as native does.
- Two inputs are not EventClass records offline and stay local frame-ingress inputs:
  selection (client-local in native) and the in-game speed, which the Options dialog
  writes directly when GameMode is 0 or 5 (`0x004E1DE0`). Opcode `0x0D` carries the
  speed only in multiplayer (stage 6).

Native records by command (EventClass constructors and Execute arms in `gamemd.exe`):

| VERA command | Opcode | Constructor | Notes |
|---|---|---|---|
| QueueProduction | PRODUCE `0x0E` | `0x004C6970` | Type RTTI, `+0xDF8` index, naval flag. |
| SuspendProduction | SUSPEND `0x0F` | `0x004C6970` | Names the category's active type; Execute uses the index only to pick Building or Defense. |
| CancelProductionByType | ABANDON `0x10` / ABANDON_ALL `0x2E` | `0x004C6970` | Also raised by `StripClass::Recalculate` (`0x006AA600`). |
| PlaceReadyBuilding | PLACE `0x0B` | `0x004C6AE0` | Object RTTI 6, index, naval, Cell. |
| PlaceProducedMobile | PLACE `0x0B` | `0x004C6AE0` | Raised by `StripClass::AI` (`0x006A8B30`); index `-1`, Cell (0,0). |
| LaunchSuperWeapon | SPECIAL_PLACE `0x12` | `0x004C6B60` | `[SuperWeaponTypes]` index and Cell. |
| SellBuilding | SELL `0x16` | `0x004C65E0` | Object token. |
| ToggleRepair | REPAIR `0x15` | `0x004C65E0` | Object token. Execute has no owner check; VERA's handler has one. |
| SetRally | ARCHIVE `0x1E` | `0x004C6780` | One record per factory: building token and that factory's searched Cell. |
| CycleProducerFocus | PRIMARY `0x11` | `0x004C65E0` | No native cycle. The native order is a self-click naming one factory, so input picks the factory. |
| UndeployBuilding | ARCHIVE `0x1E` then SELL `0x16` | | Native undeploy is a cell click on the building (`0x004436F0`). |
| SellWallAtCell, ExitMatch, Move, MoveToObject, Area Guard, Scatter | `0x17`, `0x13`, `0x04`, `0x07` | | Ported before stage 2. |

| Stop | IDLE `0x06` | `0x004C65E0` via `0x006FFE00` | Self token; StopCommand `0x00730EA0`. |
| Attack, ForceAttack | MEGAMISSION `0x04`, mission 1 | `0x004C6860` | Object in the target token. Ctrl only changes the click action (`0x0051F190`, `0x00738890`); the record is identical. |
| ForceAttackCell | MEGAMISSION, mission 1 | `0x004C6860` | Cell in the target token (`0x004D80E9`). |
| AttackMove | MEGAMISSION, mission `0x1D` | `0x004C6860` | Tokens as the original click; promoted at `0x006FFC10`. |
| EnterTransport, EnterBunker, RepairAtDepot, MinerReturn (refinery) | MEGAMISSION, mission 7 | `0x004C6860` | Building or transport in the destination token. EnterBunker's CAN_LOAD answer is unverified. |
| HarvestCell | MEGAMISSION, mission `0x0A` | `0x004C6860` | Cell in the destination token. |
| CaptureBuilding | MEGAMISSION, mission 8 | `0x004C6860` | Building in the destination token. |
| PlantC4 | MEGAMISSION, mission `0x11` | `0x004C6860` | Building in the destination token. |
| UnloadPassengers, ToggleInfantryDeploy, DeployMcv | MEGAMISSION mission `0x10` (self click) or DEPLOY `0x09` (Deploy key) | `0x004C6860` / `0x004C65E0` | Two encodings with different Execute gates; the producer decides. A garrison's self click puts the building in the target token. |
| EjectBunker | none found | | No native producer. |

Mismatches between VERA's commands and the native records. Each changes behavior once
the record is the only carrier:

- ForceAttack and Attack are the same record. VERA's handlers differ.
- MinerReturn: native sends a plain Enter (mission 7). VERA queues Enter and then forces
  Harvest with ForcedReturn, and has a no-refinery form with no native event.
- EjectBunker has no native producer.
- Deploy, unload and infantry deploy need the producer (self click or Deploy key) to
  choose the record.
- SetRally is one record per factory with that factory's searched Cell; VERA sends one
  shared Cell.
- UndeployBuilding is ARCHIVE then SELL; VERA's undeploy flag has no wire form.
- CycleProducerFocus has no native event; PRIMARY names one factory.
- ToggleRepair: native Execute has no owner check.
- SuspendProduction names the active product's type, which VERA must read from the
  factory at issue time.

Status (2026-10-14). Decision: each mismatch is ported to native behavior before
the switch, one gameplay change at a time. Nothing is committed; all of it is in
the working tree of `feature/networkStage2Again`.

Done and validated (full lib suite passed after each, except where noted):

- The queue and its convergence tests moved to `sim::event_queue`; opcodes `0x0C`
  and `0x22` execute for the local player only.
- PRODUCE/SUSPEND/ABANDON/ABANDON_ALL, PLACE and SPECIAL_PLACE record codecs; native
  TechnoType and super weapon indices in `TypeHandleTable`, both directions.
- Attack and ForceAttack are one `Attack` command; Execute has no alliance test.
- `Deploy` (DEPLOY `0x09`, no class gate, aircraft refused, OVER_OUT) and `Unload`
  (MEGAMISSION `0x10`) replace DeployMcv, ToggleInfantryDeploy and UnloadPassengers.
- `SetRally` names one building; ARCHIVE's Execute checks only that it is alive.
- Snapshot version is 356 for these command shape changes.
- MinerReturn, RepairAtDepot, EnterTransport and EnterBunker are one `Enter`
  command through the generic MEGAMISSION arm, with the mission-7 duplicate check
  (`0x004C729F`). The miner's pinned refinery and ForcedReturn cursor are deleted;
  the `Dock=` check moved to the click; the JSON profiles are converted. An Enter
  keeps VERA's `Building`/`Object` navigation forms its consumers read.
- The Unit destination setter's Enter arm (`0x00741C4F..0x00741E8E`) now serves
  every Building, not only `UnitRepair=` depots: a busy building becomes the
  ArchiveTarget, a free one is sent DOCKING (`0x0E`) and a `DockUnload=` pad's
  MOVE_HERE replaces the requested destination. The UnitRepair tail
  (`0x00742C20`) stays depot-only. A player Enter onto a refinery docks through it.
- An Enter on a miner unloading at its own refinery breaks the tether
  (`0x004C72F8`), and the same event's DOCKING re-admits and re-tethers it on the
  pad; the unload pays the slot and its new-order check (`0x0073E539`) hands over
  to the Enter. With no contact left, the latch drops at the next Mission_Unload
  visit's contact gate (`0x0073DEE0`), not at the order.
- The global harness pin moved by hash composition only (the dropped
  `reserved_refinery` fold); a control restored the previous value.

Residuals of the Enter chain:

- Transport and tank-bunker destinations still route to their VERA adapters
  (`legacy_entry_adapter`). Trigger: an Enter onto a transport or Tank Bunker.
- The setter's UnitAbsorb arm (`0x00741CE5`, pending entry and a NULL
  destination) and its Bunker arms (`0x00741D49`) are not ported; VERA's
  absorber entry and bunker adapter keep those destinations. Trigger: an Enter
  onto a `UnitAbsorb=` building or a bunker.
- The depot's busy-visitor test is VERA's `!is_full` health check, not native's
  ratio against ConditionGreen (`0x00741D83..0x00741D9B`). Trigger: a busy depot
  ordered for a unit between ConditionGreen and full health.

Ordered tasks, one session each, in this checkout:

1. Done: the Enter chain (above).
2. Building click orders: EjectBunker has no native producer (a bunkered tank is
   ordered itself); Construction Yard undeploy is a cell click sending ARCHIVE then
   SELL (`0x004436F0`), and the Deploy key on it sends DEPLOY, which queues Guard;
   CycleProducerFocus becomes PRIMARY `0x11` on one factory (native self-click on a
   non-primary factory, Execute `vt+0x4F0`); ToggleRepair's Execute has no owner check.
3. Remaining record codecs and semantic encode/decode for every command: target
   events (IDLE `0x06`, DEPLOY `0x09`, REPAIR `0x15`, SELL `0x16`, PRIMARY `0x11`),
   ARCHIVE `0x1E`, and the MegaMission orders (Attack 1, AttackMove `0x1D`, Enter 7,
   Capture 8, Harvest `0x0A`, Unload `0x10`, PlantC4 `0x11`, Cell attack). SuspendProduction
   names the factory's active type at issue. Make each authoritative at issue time.
4. The switch: `Simulation` owns the `SynchronizedCommandQueue` in place of
   `pending_commands`; issue appends to the OutList; the frame tail runs the
   single-player Queue_AI transfer and `dispatch_due_offline` with native house
   eligibility; Select and offline game speed stay local frame-ingress inputs; delete
   `apply_due_commands`, `take_due_commands` and the app due-command handoff; migrate
   the tests.
5. PR readiness: full lib suite and Clippy with retail INIs, `python -m tools.run_tests`,
   a release retail map load, one critic review, and this section rewritten as done
   with residuals.

### Stage 3: UDP transport and network lockstep

Scope: `net::protocol` with a `Transport` abstraction over a UDP socket bound to a
configurable address, loopback for the CnCNet bridge and the LAN interface for native
play. Wire format: a small versioned little-endian header (protocol version, game id,
player id, packet kind) followed by lockstep command records. Peer table keyed by
player id with an address per peer. Enable `transfer_network` and `dispatch_due_network`, exchange
FrameInfo, compare checksums, raise a desync event.

Done when: two VERA20k instances on one machine, started from a test harness or a
command-line flag, play a scenario in lockstep and detect an injected desync.

Evidence: a headless two-process test plus side-by-side play.

### Stage 4: multi-human match bootstrap

Scope: the loading path builds houses from a negotiated player list instead of one
local human plus AI slots. Each human gets its house index, side, color, spawn
location and spectator flag from the launch descriptor. Seed comes from the host.

Done when: the two instances from stage 3 load the same map with different local
players and identical world state (stage 0 checksum).

### Stage 5: `spawn.ini` launch and CnCNet client bridge

Scope: a `-SPAWN` style command-line flag that reads `spawn.ini` from the game
directory, builds the launch descriptor from `[Settings]`, `[OtherN]`, `[House*]`,
`[SpawnLocations]` and `[IsSpectator]`, points the transport at `[Tunnel]` on loopback,
and starts the match directly. Parse with the native readers in `src/rules/ini_value.rs`.
Confirm the bridge addressing facts listed above first.

Done when: a VERA20k-configured CnCNet client hosts a game, launches VERA20k on two
machines through a tunnel, and both play to completion.

Deliverable outside the repo: a client configuration package (executable name, game
id, `GameOptions.ini` with `[ForcedSpawnIniOptions]`) kept in its own repository.

### Stage 6: in-game network service

Scope: per-frame send and receive inside `advance_in_game_runtime`, the ahead-window
pacing and frame-send-rate policy, timeout and player-drop handling, and opcode `0x13`
EXIT routing for local versus remote players, which `net::lockstep` leaves to the
session owner. Reconnect is not native and is out of scope.

Done when: a player quitting or losing connection is removed as in native, the others
continue, and the match reports its result.

This stage completes the initial game port.

### Stage 7: LAN discovery and native LAN play

Scope: `net::discovery` announces a hosted game by UDP broadcast on a fixed port
(game id, protocol version, host name, map, mode, player count, game port) and lists
announcements heard on that port, with expiry. Host and join screens built on the
skirmish shell: game list, player list, sides, colors, ready state, chat, and map and
options broadcast from the host. Join negotiates the player list and seed, then
stage 4 bootstraps and stage 3 carries the match over the LAN endpoints. The
main-menu Network route replaces `bounce_network_to_main_menu` here, taking the role
of gamemd's IPX game list and lobby (state 0x10 onward from `0x0052DD75`). Session
mode is `Lan`.

Done when: two machines on one LAN host, see, join and play a full match from the
app with no CnCNet client, tunnel, internet access or configuration file.

Evidence: side-by-side play on two machines, plus a two-process discovery test on one
host.

### Stage 8: built-in tunnel client

Scope: `net::tunnel` fetches the master list, parses the twelve-field lines, pings
tunnels with the 50-byte probe and sends game traffic with the 8-byte v3 header. Peer
addresses in the stage 3 peer table become tunnel ids. The stage 7 lobby gains a
direct-IP join for internet play without a lobby server.

Done when: two VERA20k instances play through a public or self-hosted tunnel with no
CnCNet client running.

Before using public tunnels under a new game id, ask the CnCNet team. The tunnel
server is open source and self-hosting is the fallback.

### Stage 9: in-app internet lobby

Scope: a CnCNet IRC lobby speaking the client's dialect under a VERA20k game
identifier, so VERA20k games never appear joinable to gamemd players. It reuses the
stage 7 lobby screens with the game list fed from IRC instead of broadcast.

Done when: a full internet match is set up and played without any external program.

## Validation per stage

- Determinism: stage 0 checksum test runs in every later stage's suite.
- Transport and discovery: headless two-process tests in `src/net/`, never against retail assets in the lib suite.
- LAN: side-by-side play on two machines before stage 7 merges.
- Gameplay: side-by-side play against retail for anything the lockstep semantics change.
- Each PR states which evidence level each claim reached, per the project contract.

## Sources

- Client: <https://github.com/CnCNet/xna-cncnet-client> (`GameLobbyBase.cs`, `CnCNetGameLobby.cs`, `CnCNetTunnel.cs`)
- Tunnel server: <https://github.com/CnCNet/cncnet-server> (`CnCNet/Net/Tunnel/TunnelV3.cs`)
- YR spawner: <https://github.com/CnCNet/yrpp-spawner>
- Master list: <http://cncnet.org/master-list>
- v3 tunnel design notes: <https://opencollective.com/xna-cncnet-client-v3-tunnels>
