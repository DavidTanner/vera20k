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
- IPX. gamemd's Network button constructs `IPXInterfaceClass` (`0x007B0D90`) and
  opens an `AF_IPX` socket (`0x007B10C0`). Retail LAN is IPX only: no INI, registry
  or command-line key selects UDP for it, and `UDPInterfaceClass` is built only by
  the WOL route (`Network__InitUdpSession`, `0x0052F3F0`). Modern systems have no
  IPX, so VERA20k replaces it with UDP behind the same role. This is a recorded,
  necessary divergence from native.
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
CnCNet lobby, which statistics or result packets the client expects after the match, and
which native game mode the YR spawner selects. Mode 3 (LAN timing table) is likely but
unconfirmed. It decides the timeout table and which mode-4 paths stay dormant.
The authoritative files are `V3GameTunnelHandler.cs`, `TunnelHandler.cs` and
`GameProcessLogic.cs`.

## Current state in the repository

| Area | State |
|---|---|
| `src/net/lockstep.rs` | Ported native command staging and dispatch: send policies, FrameInfo (opcode `0x1c`), checksum history, single-player and network transfer. Test-only; no production caller. |
| `src/net/lockstep_sim_convergence_tests.rs` | Convergence tests over the queue, including the stage 0 two-instance test: live semantic commands against the same orders as native records (Move, Area Guard, Scatter), with sync checksums compared every frame and an injected desync caught. |
| `src/sim/multiplayer_checksum.rs` | Stage 0: per-frame sync checksum ported from `0x0064DAB0`, computed in the frame tail before command dispatch only on the multiplayer `Queue_AI` route (`CommandQueueRoute`, which the app writes from `MatchSession` before each frame). `MatchSession` records it into the `MultiplayerChecksumHistory` ring, with the native frame-0 ring reset. Offline frames never compute it. Its parity is bounded by VERA's object registries (open display-layer writers in `display_registry.rs`). |
| `MatchSession` (`src/app/match_runtime/session.rs`) | Stage 1 done: the live session owner (mode, participant list, local participant, host flag). The launch path writes Campaign or Skirmish and the saved-game restore follows the persisted classification; the modal pump, elapsed clock and command bar read it. No writer produces `Lan` or `Wol` yet. |
| `src/app/frontend/skirmish_session.rs` | Reads the local multiplayer preferences (handle, game mode) that native refreshes before the Network dialog. |
| `src/skirmish_launch.rs` | Pre-fill plan already models several human nodes plus AI slots. |
| Main menu Network button | `bounce_network_to_main_menu` in `src/app/shell_main_menu.rs` reproduces native's silent return when the IPX socket fails. Native takes the same exit (`0x0052E425`: GameMode 0, state 0x12, no message) when the player backs out of the LAN lobby. |
| Sockets, protocol, lobby | None. `src/net/mod.rs` reserves `protocol`. |

## Native reference

Mapped from the shared Ghidra database `gamemd-shared-2026-10-13` by reading
instructions and decompiled bodies. Nothing was executed. Claims marked *lead* rest on
a name, a comment or an incomplete writer scan. Clock values are in 16 ms buckets
(`WallClock__Get16msBucket`, `0x006C8C40`, `timeGetTime() >> 4`). Game modes: 0
campaign, 3 LAN (IPX), 4 internet (WOL), 5 skirmish; modes 1 and 2 are the serial
and modem routes.

### Frame order (`Main_Tick`, `0x0055D360`)

The frame runs in this order:

1. Throttle. Under protocol 2 it waits 60 / RequestedFPS buckets.
2. Input and `Process_Command`.
3. Render.
4. `LogicClass__PerTickUpdate` (`0x0055DC9E`).
5. Flush the staged events.
6. `Process_Command_Queues_For_Frame` (`0x00647260`, call at `0x0055DE40`).
7. Flush, then `Network_ServiceLoop` (`0x0048D080`).
8. Increment the frame, unless exit, victory, defeat or restart is pending.

Frame N's checksum is therefore taken after logic N, and frame N's DoList runs after
logic N.

### Command queues and checksum

- `Process_Command_Queues_For_Frame` sends modes 0 and 5 to the local OutList drain,
  and modes 1 to 4 to `Process_Network_Command_Queues_For_Frame` (`0x006475F0`,
  switch at `0x006475C8`).
- `Compute_Game_Sync_Checksum` (`0x0064DAB0`) has exactly two call sites:
  - `0x00647684` on the network path, every frame.
  - `0x0064731C` on replay playback (`g_dwReplayModeFlags` `0x00A8D5F8 & 2`) in any
    mode. That branch also writes a sync report once the frame reaches
    `[SyncBug] PrintCRC` (Session +0x30D4, default `0x7FFFFFFF`).
- Ring reset (`0x00647825`) runs when the frame is 0 or `[0x00A8B8B8]` (Session +0x680)
  is set. It zeroes the 256-slot ring and the seven 0x18-byte per-connection records
  at `0x00AFA358`:

  | Offset | Field |
  |---|---|
  | +0 | their frame |
  | +4 | their sent |
  | +8 | their received |
  | +0xC | FrameSync round trip |
  | +0x10 | router response |
  | +0x14 | heartbeat ms |

  It then sends FrameSync `0x19` (`FUN_0064A000`) and runs the first-time player wait
  (`FUN_00648710`, ECX=1).
- `[0x00A8B8B8]` is a one-shot flag:
  - It is cleared after that first wait.
  - It skips `Start_Scenario` (`0x0052E631`) and reseeding (`0x0052FC33`).
  - No nonzero writer was found by an absolute-address scan. Saved-game or rejoin use
    is a *lead*.
  - It is not a timing control.

### Timing: MaxAhead and FrameSendRate

| Global | Address |
|---|---|
| MaxAhead (`g_NetworkFrameBudget`) | `0x00A8B550` |
| FrameSendRate (FSR) | `0x00A8B554` |
| RequestedFPS | `0x00A8B558` |
| MaxMaxAhead | `0x00A8B568` |
| PreCalc | `0x00A8B56C` / `0x00A8B570` |
| LatencyFudge | `0x00A8DB9C` |
| ProtocolVersion | `0x00A8B24C` |

The network path schedules four kinds of timing work:

- **Every 128 frames:**
  - It reads the transport response time, which is the largest per-connection +0x1C
    from `IPXManagerClass__ResponseTime` (`0x00542450`).
  - It sets the transport retry and timeout to (rt+10, -1, max(rt·8+15, 120)).
  - It queues PROCESS_TIME `0x21` with the average milliseconds per frame from Main_Tick.
- **Host only (`Am_I_Master`), every 256 frames or when PreCalc is set:** it queues
  TIMING `0x20`, which carries:
  - FPS: the minimum of the peers' PROCESS_TIME, capped by game speed.
  - MaxAhead: rt·FPS/120 scaled by LatencyFudge. Growth is capped at MaxAhead + FSR,
    rounded up to FSR, at least 3·FSR and at most about 256.
  - FSR.
- **Writers:** TIMING's execute arm writes MaxAhead, FSR and FPS. RESPONSE_TIME `0x1B`
  sets MaxAhead from byte +0xD.
- **When MaxAhead or FSR grows,** TIMING sets the window `[0x00A8B1F8, 0x00A8B1DC]`. It
  runs from the event frame to roundup(frame + MaxAhead, FSR). `Execute_DoList`
  restamps queued records inside it.
- **Under protocol 2, frames that are not FSR boundaries** service the transport and
  return. They send nothing, wait for nothing and run no DoList.
- **Not established:** who first writes MaxAhead and FSR at session setup.

### Waiting for peers and timeouts

The OutList is packetized by `FUN_00649CA0` (`0x00647FB4`): FrameInfo `0x1C`, then
the commands stamped frame + MaxAhead. `FUN_00648710` (ECX=0, `0x00648067`) then
waits. During the wait, input and rendering keep running and the simulation stalls.

- **Release:** it returns once the frame is below min(their frame) + MaxAhead and every
  peer's received count has caught up with its sent count.
- **Dialog expiry:** dialog 0xEA, "Reconnecting to <slowest>", with a countdown.
- **Timeout, first wait:** returns 7 (system not responding) and the game ends.
- **Timeout, later waits:** `FUN_0064B410` kicks the lowest-frame connection
  (`0x0064B452..0x0064B47B`). The timers reset and the wait continues.
- **Player abort:** returns 8 or 9. A vote-kick also exists (`[0x00A8BB40]`).

Per-mode table at `0x00837340`, 16 bytes per mode, values in buckets:

| Mode | Resend | First dialog | Dialog | Timeout |
|---|---|---|---|---|
| 1, 2 | 120 | 900 | 420 | 3600 |
| 3 (LAN) | 60 | 900 | 420 | 2400 (≈38 s) |
| 4 (WOL) | 120 | 1800 | 720 | 7200 |

`Execute_DoList` (`0x0064C380`, call at `0x0064822F`) skips FrameInfo checksum
comparisons while timer `0x00AFA450` still has time left. That timer is set at the
reset to start at frame and run 0x100. Because its duration is stored as an absolute
frame, the skip lasts longer than 256 frames. This is a native quirk to keep.

### Player removal (`Network__DestroyConnection`, `0x005DA750`)

| Caller | Trigger | Reason |
|---|---|---|
| `0x0048D859` (`Network__ProcessGlobalPackets`) | Global disconnect packet `0x0A` | 0 |
| `0x0064B3FE` (`FUN_0064B240`, kick) | Wait timeout, or a vote-kick | 1, or 0 for a local outvote |
| `0x0064C9B1` (`Execute_DoList`) | Remote EXIT `0x13` | 0 |
| `0x00684596` (`Network__WaitForScenarioLoadCompletion`) | Load progress below 0.999 at the load timeout | 1 |

- **Reason 1:** queues REMOVEPLAYER `0x23`, or on frame 0 calls `Flag_To_Die` directly.
  REMOVEPLAYER's execute arm defeats the human house in sync.
- **Reason 0:** shows the left-game message and removes the connection and the node.
  There is no defeat (outside a mode-4 tournament) and no AI takeover. AI conversion
  happens only under replay flag 2 (`0x0064C9F0`) or in the modem modes.

EXIT (`Queue_Exit`, `0x006471A0`) queues `0x13`. In mode 4 with `[0x00A8DBA0]`, more
than two players and this client as master, it queues ABOUTTOEXIT `0x27` instead.

In `Execute_DoList`, a due `0x13` record does the following:

1. It sets House +0x248.
2. If no other undefeated human remains, it calls `Flag_To_Die`.
3. For the local house, it runs `EventClass__Execute`, which sets
   `g_bScenarioExitRequested`.
4. For a remote house in modes 3 and 4, it compacts the records and calls
   `DestroyConnection(house, 0)`.

The rewrite gate at `0x00A8E2D8`/`DC`/`E0` is written only by ABOUTTOEXIT. It is
mode-4 host migration and dormant for LAN.

`Network_Keepalive` (`0x00542520`) is misnamed. It gathers per-connection statistics
in mode 4 only, on frames where `frame & 7 == 7`. The mode-4 heartbeat is the
`NET_IM_STILL_HERE` global packet, recorded at record +0x14 by `0x00646DF0`.

### Match bootstrap

- **Session nodes:** `DynamicVectorClass<NodeNameType*>` at `0x00A8DA78`, with the
  count at `0x00A8DA84`.

  | Offset | Field |
  |---|---|
  | +0x00 | wide name |
  | +0x28 | address |
  | +0x4B / +0x4F | country / random marker (-2 random, -3 observer) |
  | +0x53 / +0x57 | color / marker |
  | +0x5B / +0x5F | start / marker |
  | +0x63 / +0x67 | team (becomes House starting allies) / marker |
  | +0x6B | -1 marks an observer |
  | +0x6F | assigned House index |
  | +0x73 | per-node frame-rate figure |
  | +0x7B | clan |

  No handicap field was found. AI slots are parallel arrays from `0x00A8B27C` (difficulty,
  side, color, start, team at 0x20 strides).
- **Seed:** `Init_Random_Number_System` (`0x0052FC20`) builds a time seed only in
  modes 0 and 5. Modes 1 to 4 reuse `g_RngSeed`.
  - In the LAN lobby the host sets it when dialog 0xBC opens (`0x005DEC5D`).
  - `Build_Options_String` (`0x005DBB60`) puts it first, ahead of the nodes and AI
    arrays.
  - Clients store it from `MPGameOptions__ParsePacket` (`0x005E4DC1`).
- **Random side and color** are resolved once, on the host, before start, by
  `SessionClass__ProcessRandomAssignments` (`0x0069B8C0`, called at `0x005DD3C5`).
  - It uses the Scenario RNG before reseeding.
  - Human side is `RandomRanged(0,9)` unless the game mode overrides it. Human color
    is `RandomRanged(0,7)`, retried until free. Observers get color 8.
  - The host then rebroadcasts the options.
- **Random start positions** are resolved on every machine during load
  (`MultiplayerGameMode__AssignStartingPositionsToHouse`, `0x005D6890`), after
  seeding. Which RNG it uses is not established.
- **House build:** `ScenarioClass__Create_Houses` (`0x00687F10`).
  - It builds humans in ascending node color, keeping node order on ties. Node 0
    becomes the player.
  - Then it builds valid AI slots, then Neutral, then Special.
  - It writes node +0x6F.
- **After load:**
  - `SessionClass__CreateConnections` (`0x00697B70`, modes 3 and 4) maps nodes to
    houses by name.
  - `SessionClass__Apply_Squad_Alliances` (`0x0069B170`) runs in mode 4 only.
- **Load wait:** `Network__WaitForScenarioLoadCompletion` (`0x00684370`).
  - The timeout starts at 3600 and resets to 7200 whenever progress changes.
  - Progress is re-sent after 300, then every 60.
  - Laggards below 0.999 are dropped with reason 1. The match continues without them.

### LAN route and lobby

- **Entry:** `Main__PrepareSession` (`0x0052D9A0`) handles the Network button at
  `0x0052DD75` (address mode 1, GameMode 3, state 0x10). WOL is `0x0052DD57` (address
  mode 2, GameMode 4). State 0x10 is shared by modes 3, 4 and 5. The LAN setup is its
  GameMode-3 branch at `0x0052E35D`, which does the following in order:
  1. `ReadMultiPlayerSettings`.
  2. Constructs `IPXInterfaceClass`.
  3. Calls `MultiplayerLobby__Initialize` (`0x005DB680`).
  4. Calls `LANLobby__Run` (`0x005DC350`).
- **Product tag:** only packets carrying `0xAA04` are processed.
- **Discovery:**
  - Clients that are not hosting broadcast query type 0 every 120 buckets (1.92 s).
  - Hosts answer with unicast type 1 (`Network__HandleSessionQueryPacket`,
    `0x005DFE00`), carrying the game name and an open bit. Hosts keep answering
    in-game, so the game shows as in progress.
  - On an open-bit change, `0x005E00B0` shows "formed new game" or "game now in
    progress".
  - `-STEALTH` suppresses the replies.
- **Expiry** (`LANLobby__ProcessTimeouts`, `0x005DB720`):
  - A listed game unrefreshed for more than 300 buckets (4.8 s) is dropped; the
    player's own game is exempt.
  - A selected game is polled with type 2 every 35 buckets, and the host answers with
    type 3.
  - Lobby users announce with type 4 every 83 buckets. A user silent for 300 buckets
    gets one type 5 ping, and is removed after 361 buckets (≈5.8 s).
- **Join** (`0x005DF410`):
  - The client sends type 6 with name, side/color and version range, and times out
    after 600 buckets with "request denied".
  - Type 7 means accepted. Type 8 means rejected (reason 0: name must be unique).
  - Type 10 means a player or game left.
  - Types 0x16 to 0x18 carry options.
- **Start** (host button 0x588 in `LANLobby__Run`):
  - It requires at least two non-observers, every client holding the map, a scenario
    large enough, legal alliances and every player accepted.
  - It then sends options `0x16` and GO `0xB` with the frame budget, and drains the
    send queue for up to 1200 buckets.
  - It waits up to 600 buckets per client for `0x10` (request scenario) or `0x13`
    (ready). Map transfer is `SessionClass__SendFileToClients` (`0x00694760`).
- **Local preferences:** `RA2MD.INI` `[MultiPlayer]` (`0x006980C0`): Handle, Color,
  ColorEx, Side, SideEx, LANTaunts, LANScrollText, SendDelay, PortNumberOverride.
  PortBase, ForcePortBase and PortPool apply only to the UDP (WOL) route.
- **Command line:** `-SOCKET<n>`, `-DESTNET` and `-STEALTH` (`Parse_Command_Line`,
  `0x0052F620`).

## Target ownership

`sim/` stays transport-agnostic. `net/` depends on `sim/` only. App code orchestrates.

| Responsibility | Owner |
|---|---|
| Command staging, frame stamps, dispatch order, checksum history | `net::lockstep` (exists) |
| Wire framing, peer table, send and receive, tunnel header | `net::protocol` (new) |
| LAN discovery: announce, listen, game list | `net::discovery` (new, stage 7) |
| Tunnel discovery: master list, ping, selection | `net::tunnel` (new, stage 8) |
| Session mode, player list, local player id, host flag | `app::match_runtime::session::MatchSession` (exists) |
| `spawn.ini` parsing into a launch descriptor | `app/frontend` launch path, reusing `skirmish_launch` types |
| Per-frame network service in the game loop | `app/match_runtime/sim_tick` (stage 6) |
| Lobby UI | `ui/` on top of the skirmish shell (stages 7 and 9) |

Rules that apply throughout: one owner per state, no competing command paths, every
RNG draw and timer in native frame order, presentation never reads the socket.

## Implementation chains, in dependency order

Each stage is one PR. A stage is done when its "Done when" holds and the usual lib
suite passes. Stages 1 to 6 produce the initial game port: a VERA20k that the
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
- Replay playback (`g_dwReplayModeFlags` `0x00A8D5F8 & 2`) also computes the checksum,
  in any mode (`0x0064731C`). VERA has no production playback route.
- The ring reset's second trigger, `[0x00A8B8B8]`, is a one-shot load or rejoin flag,
  not timing (see [Native reference](#command-queues-and-checksum)). VERA has no
  writer for it. It belongs with saved multiplayer games, which no stage covers yet.

Why first: every later stage debugs against this. A desync found after the transport
exists is far harder to attribute.

### Stage 1: live session mode (done)

Scope: replace the hard-coded `current_session_mode` with a value the launch path
writes. Introduce the session owner holding mode, player list, local player index and
host flag. Skirmish and campaign keep writing their modes.

Done when: the modal pump, elapsed clock and any other `is_network` consumer read the
live value, and offline play is unchanged.

### Stage 2: lockstep queue in production, offline path

Goal: route live command issue through `SynchronizedCommandQueue`, using
`transfer_single_player` and `dispatch_due_offline`. Delete the direct command path it
replaces. Keep RNG and same-frame ordering identical; the convergence tests and the
stage 0 checksum test guard this.

Done when: skirmish and campaign play through the queue with no behavior change, and
the old path is gone.

Why before transport: the queue's frame-stamp and ahead-window semantics are native
behavior the game must already obey before a second client exists.

The queue carries only native 0x6F-byte `EventClass` records and accepts no semantic
sidecar, so every live `Command` needs a native encoding and decoding first. Before 2a the
app and the sim both append semantic `CommandEnvelope`s to `Simulation::pending_commands`,
which the frame tail already dispatches in native house order with MegaMission
staging. Only Move, MoveToObject, Attack/ForceAttack, ForceAttackCell, AttackMove,
Guard, Scatter, SellWallAtCell, ExitMatch and the planning records are encoded.
gamemd's event name table (`0x008209DC`) lists 47 opcodes, `0x00` EMPTY to `0x2E`
ABANDON_ALL. Stage 2 is therefore split into one PR per command family, then a
switchover. Each family PR ports the encoding and decoding, and makes local issue
queue what the record decodes to. Execution differences the bytes cannot carry are
resolved toward native in that PR:

| Part | Commands | Native events |
|---|---|---|
| 2a MegaMission | Attack, ForceAttack, ForceAttackCell, AttackMove, EnterTransport, EnterBunker, RepairAtDepot, MinerReturn, CaptureBuilding, PlantC4, HarvestCell, UnloadPassengers, ToggleInfantryDeploy | MEGAMISSION `0x04`: Attack 1, AttackMove 29, Enter 7, Capture 8, Harvest 10, Unload 16, Sabotage 17 |
| 2b Production | QueueProduction, SuspendProduction, CancelProductionByType, PlaceReadyBuilding, PlaceProducedMobile, CycleProducerFocus | PRODUCE `0x0E`, SUSPEND `0x0F`, ABANDON `0x10`, ABANDON_ALL `0x2E`, PLACE `0x0B`, PRIMARY `0x11` (CycleProducerFocus sends its chosen factory's PRIMARY) |
| 2c Single events | Stop, DeployMcv, SellBuilding, ToggleRepair, LaunchSuperWeapon, SetGameSpeed, SetRally, UndeployBuilding, EjectBunker, and the deploy key's UnloadPassengers and ToggleInfantryDeploy | IDLE `0x06`, DEPLOY `0x09` (the deploy key), SELL `0x16`, REPAIR `0x15`, SPECIAL_PLACE `0x12`, GAMESPEED `0x0D` (network games only), ARCHIVE `0x1E` for rally; UndeployBuilding sends ARCHIVE then SELL; the MCV self-click and EjectBunker send MEGAMISSION Unload16 |
| 2d Local selection | Select | None: native selection is client-local and never an event |
| 2e Switchover | Every producer, including sim-generated strip PLACE and ABANDON | The queue becomes the only command path; `pending_commands` is deleted |

Each part's mapping is checked against the native producer and `EventClass::Execute`
before it is ported.

#### 2a: MegaMission family

Native producers: Foot `ClickedAction` (`0x004D74E0` object, `0x004D7D50` Cell) and
Building `ClickedAction` (`0x00443410`) call `ClickedMission` (`0x006FFBE0`), which
queues one MEGAMISSION record of (mission, target, destination):

| Click | Mission | Target | Destination |
|---|---|---|---|
| Enter (action 3) on an object | 7 | NULL | the object |
| Capture (action 9) | 8 | NULL | the building |
| Sabotage (action 0x10, 0x1A) | 17 | NULL | the building |
| Harvest (Cell action 6) | 10 | NULL | the Cell |
| Foot self-click (action 4) | 16 | NULL | NULL |
| Building self-click (action 4), garrison or absorber | 16 | the building | NULL |
| Grinder (action 0xB) | 9 Eaten | NULL | the building |

The receiver (`EventClass::Execute` `0x004C7200..0x004C74A2`) admits both tokens,
skips a repeated Enter onto the current contact, queues the mission, and hands the
target and destination to the class setters. It has no alliance test.

Delivered: the record codec carries every row above. Issue time round-trips each of
these orders through its record (`roundtrip_ordinary_local_megamission`), so the
queued command is what the bytes decode to:
- Attack decodes to ForceAttack. The VERA-only alliance check on the Attack receiver
  is removed; What_Action already chose Attack.
- Enter decodes to `Command::Enter`. Its execution picks MinerReturn, RepairAtDepot,
  EnterBunker or EnterTransport from the live destination, where native's single
  receiver leaves that to the destination's radio answer.
- Unload decodes to ToggleInfantryDeploy for infantry (`0x0051F6E0`) and to
  UnloadPassengers for every other class. 2c adds the Unit's deploy and bunker arms
  and the deploy key's DEPLOY.
- A Harvest run onto one Cell gets the group spread, like Move and AttackMove: the run
  key (`0x0064CB0A..0x0064CB60`) has no mission filter.

Residuals:
- Planning mode records only Move, Attack, AttackMove and Area Guard; other orders
  are dropped while planning, as before. Native's planning branch of
  `ClickedMission` (`0x00637AA0`) has no mission filter. Trigger: an Enter, Capture,
  Sabotage, Harvest or Unload click in planning mode.
- The represented Enter and Unload receivers keep VERA-only gates that native leaves
  to the destination's radio answer or mission: RepairAtDepot needs the same owner
  as the depot, a damaged Unit and no deploy; EnterBunker needs the same owner;
  MinerReturn re-checks the refinery; UnloadPassengers needs the same owner and
  cargo. The click already applies the same tests, so a refusal needs the state to
  change between issue and the frame tail. Effect: the order is dropped where native
  would drive to the building and be refused there.
- Building Unload covers a gap generator. The decoder maps it to UnloadPassengers,
  which refuses it. No VERA producer sends it. (The Unit's MCV and Slave Miner arm
  decodes since 2c.)
- Eaten (mission 9, the Grinder click) has no VERA producer and does not decode.
- Enter receiver selection is made twice: by the click and by `enter_receiver` at
  execution. Both are type-based and resolve in the same order.
- The garrison Unload receiver does not run the building's target setter with
  itself as the target.
- Enter onto a Cell (Cell action 3) and Sabotage onto a Cell (Cell action 0x10) have
  no VERA producer and do not decode.

#### 2b: Production family

Native producers, with the record each sends:

| Command | Producer | Event | Payload |
|---|---|---|---|
| QueueProduction | Cameo left click, `SelectClass::Action` (`0x006AB511`, `0x006AB76E`) | PRODUCE `0x0E` | Strip type RTTI, type index, type `Naval=` |
| SuspendProduction | Cameo right click on a building cameo (`0x006AB02A`, `0x006AB12B`) | SUSPEND `0x0F` | The cameo's type |
| CancelProductionByType | Cameo right click (`0x006AB1FF`, `0x006AB231`), ALL with Shift; Strip Recalculate (`0x006AA7E7`, `0x006AA8CD`); a ready building without a builder (`0x006AB349`) | ABANDON `0x10`, ABANDON_ALL `0x2E` | As PRODUCE |
| PlaceReadyBuilding | Placement click, `DisplayClass::BandBox_LeftUp` (`0x004ABB55`) | PLACE `0x0B` | Placement object's RTTI (Building 6), type index, `Naval=`, Cell |
| PlaceProducedMobile | Strip AI (`0x006A8EE8`) | PLACE `0x0B` | Completed object's RTTI (1, 2, `0x0F`), type index -1, `Naval=`, the empty Cell |
| CycleProducerFocus | None: VERA's sidebar Factory button. A factory self-click (Building ClickedAction `0x00443494`) is native's PRIMARY producer | PRIMARY `0x11` | The building's Abstract token, from its house |

Constructor `0x004C6970` writes the RTTI, type index and naval dwords at Event+7,
+0xB and +0xF; `0x004C6AE0` adds the Cell at +0x13 and +0x15; PRIMARY uses the
Target constructor `0x004C65E0`, as Scatter does. None writes another byte.
`EventClass::Execute` (`0x004C6CB0`) hands the three dwords to
`HouseClass::Begin_Production` (`0x004FA350`), `Suspend_Production` (`0x004FA910`),
`Abandon_Production` (`0x004FAA10`, `bAll` for ABANDON_ALL) and `Place_Production`
(`0x004FB0E0`), the naval dword as `!= 0`, and PRIMARY's live building to
`BuildingClass::TogglePrimaryFactory` (`0x00448160`) without a house check. Each
House receiver accepts the object or the type RTTI of the same TechnoType array
(`RTTI_To_TypeArray` `0x0048DCD0`, table `0x0048DD24`: Unit 1 or UnitType `0x28`,
Aircraft 2 or 3, Building 6 or 7, Infantry `0x0F` or `0x10`) and picks its factory
slot from it, the type's `BuildCat=` and, for a Unit, the naval dword.

Delivered:
- Record codecs `ProductionEventRecord` and `PlaceRecord`; Scatter's record becomes
  `TargetEventRecord`, which also carries PRIMARY.
- `Simulation::encode_production_record` builds each record as its producer would,
  with type indices from `RuleSet::type_array_index`. Decoding production records
  needs the rules, so `decode_native_command_record` and `decode_for_simulation`
  take them.
- Issue round-trips every production command through its record
  (`schedule_command_in_sim`), which takes the match's rules.
- SUSPEND names the type its factory is building and decodes to the slot, the only
  thing its receiver reads.
- The Factory button chooses the category's next factory, as before, and sends
  that building's PRIMARY. It decodes to the new `Command::TogglePrimaryFactory`,
  a port of `TogglePrimaryFactory`: a primary factory clears its flag, any other
  becomes its slot's primary and, when its house is the player's, speaks
  `EVA_PrimaryBuildingSelected` (no section in retail `evamd.ini`, so silent, as
  natively). The button sends nothing when its choice is already primary (a single
  factory), because PRIMARY would clear it.
- The strip's own PLACE and ABANDON events encode through the same encoder.
  They are still queued as commands; routing them through the queue is 2e.

Residuals:
- A Unit record whose naval dword disagrees with the type's `Naval=` does not
  decode: native would use the other factory slot, VERA's receivers take the slot
  from the type. No producer sends one.
- PLACE decodes only its producer forms. A building with type index -1, a mobile
  with a type index, and a mobile with a Cell do not decode. Native would place the
  slot's object at the Cell, or send a mobile out but abandon its factory
  (`Abandon_Production`) only when the object's index matches. No producer sends
  them.
- SUSPEND names the factory's current product where native names the clicked
  cameo's type. They differ only for a cameo linked to a rearmed factory; the
  receiver reads only the slot, so execution is the same and only the bytes differ.
- A left click on a completed mobile cameo (`0x006AB3FC`: type RTTI, index -1)
  has no VERA producer; Strip AI places mobiles first. The decoder accepts it.
- A factory self-click has no VERA producer; only the Factory button sends PRIMARY.
- The button chooses from the primary as it stands at issue, so two presses
  before the first PRIMARY executes name the same building and the second clears
  it. Trigger: two presses in one issue-to-execution window (a frame offline, the
  ahead window in network play). Effect: the slot has no primary until the next
  press, and FindFactory falls back to the last eligible factory.
- Issue now needs the type to resolve to a native array index: a production
  command naming an unknown type is dropped at issue rather than refused by its
  receiver, and the queued type id is the rules' own spelling of the name.
- `TogglePrimaryFactory` does not make a building factory's call to `0x0050DB00`
  (`0x00448241`, Ghidra's DetermineEdge, a name lead), and VERA keeps one primary per slot where native keeps a
  byte per building and skips limbo buildings when clearing (`0x004481AF`), so a
  primary building in limbo loses it. Trigger: a Construction Yard made primary,
  or a factory toggled while its slot's primary is in limbo.

#### 2c: Single events

Native producers, with the record each sends:

| Command | Producer | Event | Payload |
|---|---|---|---|
| Stop | Stop key (`0x00730EA0`, event 6 at `0x00730EE7` -> ClickedEvent `0x006FFE00`) | IDLE `0x06` | The object's token, from the player's house |
| DeployMcv, ToggleInfantryDeploy, UnloadPassengers (deploy key) | `Selection::IssueDeployCommand` (`0x00730AF0`, `push 9` at `0x00730D10` -> ClickedEvent) | DEPLOY `0x09` | As IDLE |
| DeployMcv (self-click) | Foot ClickedAction action 4 (`0x004D74E0`) | MEGAMISSION Unload16 | NULL target and destination (the 2a codec) |
| EjectBunker | None: VERA's click on the bunker. Native empties a tank bunker when the occupant runs Unload (`0x0073D630` -> `EmptyBunker`) | MEGAMISSION Unload16 | The occupant as source, NULL tokens |
| SellBuilding | Sell click (`DisplayClass::BandBox_LeftUp`, action 0xC, `0x004AC187`) | SELL `0x16` | As IDLE |
| ToggleRepair | Repair click (action 10, `0x004AC187`) | REPAIR `0x15` | As IDLE |
| LaunchSuperWeapon | Targeting click (`0x004AC23A`) | SPECIAL_PLACE `0x12` | Superweapon type index, Cell |
| SetRally | `BuildingClass::SetRallyPoint` (`0x00443860`) per factory | ARCHIVE `0x1E` | The factory's token and the Cell's, from the factory's house |
| UndeployBuilding | None: VERA's self-click and deploy key. Native undeploys only by the cell click (Building Active_Click_With `0x004436F0`) | ARCHIVE, then SELL (`0x004437F0`) | The building and a Cell, then the building, from its house |
| SetGameSpeed | In-game Options (`OptionsClass::ApplyFromInGameDialog 0x004E1E54`), network modes only | GAMESPEED `0x0D` | The speed dword |

IDLE, DEPLOY, SELL and REPAIR use the Target constructor `0x004C65E0`. ARCHIVE uses
the two-target constructor `0x004C6780` (tokens at Event+7 and +0xC, kinds at +0xB and
+0x10). SPECIAL_PLACE uses `0x004C6B60` (dword at +7, Cell at +0xB and +0xD).
GAMESPEED uses the one-dword constructor `0x004C6720` (+7). None writes another
byte. `EventClass::Execute` arms (jump table `0x004C8114`):

- IDLE `0x004C74CB`: any live, untethered Techno not in Construction or Selling, no
  house test.
- DEPLOY `0x004C76BC`: for every class it refuses an EMP'd, tethered or limbo
  object, a slope, Construction and Selling, Aircraft (`0x004C7781`) and an object on
  a WeaponsFactory cell (`0x004C778A..0x004C77DE`). Then it sends OVER_OUT, a NULL
  destination, a NULL target and `Queue_Mission(Unload)`. The class's Mission_Unload does the rest: Infantry
  `0x0051F6E0` toggles deploy-fire; Unit `0x0073D630` empties a tank bunker, unloads
  a transport type, or deploys a `DeploysInto=` or simple-deployer type that is not
  `Harvester=` or `Weeder=`; Building `0x0044D880` ejects occupants or absorbed
  passengers and never undeploys.
- SELL `0x004C6F12`: the object's owner must be the event's house; a building takes
  `Sell_Back(-1)` (`0x00447110`), and so does a Unit or Aircraft standing on a
  building.
- REPAIR `0x004C6ED2`: any live Techno, `ToggleRepair(-1)`, no house test.
- SPECIAL_PLACE `0x004C78D6`: `HouseClass::Fire_SW(house, index, Cell)`.
- GAMESPEED `0x004C794B`: stores the dword as the game speed and prints a message
  for another house's change.
- ARCHIVE `0x004C6DAA`: any live Techno's `Set_ArchiveTarget`, no house or type test.

Delivered:
- Record codecs `ArchiveRecord`, `SpecialPlaceRecord` and `IntEventRecord`;
  `TargetEventRecord` also carries IDLE, DEPLOY, SELL and REPAIR.
- `Simulation::encode_single_event_records` builds each record as its producer
  would; `encode_target_event_record` replaces the Scatter and PRIMARY encoders.
  `encode_megamission_record` also sends the MCV self-click and the bunker eject.
- `decode_native_command_record` decodes each event. DEPLOY and MEGAMISSION Unload
  share one receiver choice (`unload_mission_command`), which needs the rules for a
  Unit: Infantry -> ToggleInfantryDeploy; Unit in a bunker -> EjectBunker; Unit that
  deploys -> DeployMcv; anything else -> UnloadPassengers. DEPLOY on an Aircraft
  decodes to nothing. SPECIAL_PLACE decodes only with the rules.
- Issue round-trips every 2c command. `schedule_command_in_sim` (keys, sidebar,
  targeting) sends DEPLOY for the deploy family, so 2a's deploy-key exemption is gone.
  `roundtrip_local_order` (the click route; it replaces
  `roundtrip_ordinary_local_megamission` and takes the rules) sends the self-click
  Unload, one ARCHIVE per rally factory, and ARCHIVE then SELL for an undeploy.
- Receivers moved toward native: Stop and ToggleRepair no longer check the owner, and
  SetRally archives any live object it names. The click still names only the
  player's rally factories.
- UndeployBuilding is issued as ARCHIVE of the building's own Cell (the Cell
  `SetRallyPoint`'s miss arm archives, `0x00443B17`), then SELL. The existing Selling
  mission converts an `UndeploysInto=` building that has an ArchiveTarget, so the
  yard converts on the same frame with no refund, and the unit now drives to that
  Cell.
- SetGameSpeed issue is unchanged offline: native stores the speed at once in modes 0
  and 5 and sends no event. GAMESPEED decodes to SetGameSpeed for network play. Its
  encoder arm has no offline caller; it is there for network issue in stage 3.
- A Unit deploys through the existing owners `mcv_deploy::is_mcv` and
  `unit_simple_deploy::is_simple_deployer`. `is_mcv` now also excludes `Weeder=`, as
  the refinery arm of `0x0073D630` does.

Residuals:
- UndeployBuilding and EjectBunker have no native producer. Natively, a self-click
  on a Construction Yard sends PRIMARY (it is a factory), DEPLOY on a building never
  undeploys, and a tank bunker's occupant is ejected by its own Unload. Effect: VERA's
  undeploy sends the cell click's records with the building's own Cell, so the unit
  drives to the yard's centre Cell, not a clicked Cell. VERA's eject click sends the
  occupant's Unload.
- The undeploy records skip the semantic receiver's runtime test
  (`can_undeploy_building_runtime`: build-up, MCVRedeploy, radio contacts, mind
  control, human owner). The click checks it at issue, and native's click does no
  more. Trigger: a yard gains a radio contact or a mind controller between issue and
  execution. Effect: the SELL converts the yard, or sells it where the semantic order
  was refused. Rare.
- VERA archives the building's own Cell for every `UndeploysInto=` building. Native's
  miss arm does that only for a redeployable Construction Yard. Effect: a Slave Miner
  refinery's undeploy also sends the Slave Miner to the refinery's centre Cell.
- The semantic UndeployBuilding receiver, `SellOrder::Undeploy` and
  `BuildingDown::undeploy_order` still run saved and replayed commands. Issue no
  longer reaches them. Remove them with `pending_commands` in 2e, once saved-command
  compatibility is decided.
- EjectBunker ejects at its receiver. Native ejects at the occupant's Unload visit,
  then continues into its deploy arms, so a bunkered MCV would also deploy. Trigger:
  every eject click. Effect: the occupant leaves about one mission visit earlier.
- DEPLOY's receivers keep their own gates, including 2a's owner gates, instead of
  the arm's EMP, slope, mission and WeaponsFactory tests. DeployMcv already ports
  them (`deploy::issue_order`). A DEPLOY whose class does nothing with Unload, such
  as a plain tank, decodes to UnloadPassengers, which refuses it. Native clears that
  object's target and destination and queues Unload, so it stops. No VERA producer
  sends one: the deploy key filters non-deployers. ToggleInfantryDeploy and
  UnloadPassengers lack the arm's EMP, slope, mission and WeaponsFactory tests, so a
  GI deploying on a factory Cell is accepted where native refuses it.
- The MCV self-click's MEGAMISSION decodes to DeployMcv, whose receiver applies
  DEPLOY's gates. The MEGAMISSION arm (`0x004C71CA`) has none of them. Trigger: an MCV
  self-click on a WeaponsFactory Cell or a slope. Effect: VERA refuses, where native
  queues Unload. This predates 2c.
- The Unit's deploy-fire arm (`DeployFire=` without `IsSimpleDeployer=`) and its
  refinery-unload arm (`Harvester=`, `Weeder=`) decode to UnloadPassengers, which
  refuses them. A transport type that is also a simple deployer decodes to the
  transport arm: an empty one is refused where VERA's deploy key chose DeployMcv.
  Frequency in retail data is unchecked.
- Offline, the receiver is chosen at issue, from the state at issue. Natively the
  class's Unload mission chooses it a frame later. In 2e the decode moves to the
  record's execution frame.
- SELL of a Unit or Aircraft standing on a building (sell click action 0xD) decodes
  to SellBuilding, which refuses a non-structure. An ARCHIVE naming an object, and
  the sidebar's no-target SPECIAL_PLACE (`SelectClass::Action 0x006AAF3C`, Cell 0,0),
  have no VERA producer. The object ARCHIVE does not decode.
- SPECIAL_PLACE names the selected type, where native names the first type whose
  `Action=` matches the click (`FindFirstOfAction`). Retail actions are unique.
- GAMESPEED for network play: issue must send the record and not apply the speed
  locally. Its execution belongs in the EventClass tail; `SetGameSpeed` applies at
  frame ingress. The decoder accepts only the Options range 0..=6, where native
  stores any dword, and the other-house message is not ported. 2e or stage 3 must
  give the decoded event tail timing before network play.
- In planning mode ClickedEvent hands IDLE and DEPLOY to the planner
  (`0x00639FD0`). The Stop key refuses in planning mode, as before; the deploy key
  still issues. This was not changed here.
- Every arm clears the object's planning tokens. VERA detaches planning only for
  Stop, Scatter, DeployMcv and MegaMission actors, not for SELL, REPAIR or ARCHIVE,
  which name buildings.

Nothing here blocks 2d. 2e must handle GAMESPEED's tail timing, decoding at the
execution frame, and the leftover UndeployBuilding paths.

#### 2d: Local selection

Native: selection is each object's IsSelected byte (`ObjectClass+0x83`) plus the
current-objects vector. `ObjectClass::Select` (`0x005F4520`) refuses limbo, an
already selected, unselectable, loaner or warping object and pending placement, clears
the selection when the first member's house differs in `IsHumanPlayer` from this one's
or is not human, inserts the object (in front for a positive-damage Primary, else at
the back) and sets the byte. `ObjectClass::Deselect`
(`0x005F44A0`) removes it and clears Follow. `TechnoClass::Select` (`0x006FBFA0`) wraps
it: a nonlocal undiscovered owner is refused, then the object's Tag gets event 0x21
(33, "selected"), then VoiceSelect. Local input calls Select on the spot: MainTick
`0x0055D8AB`/`0x0055D8B4` -> Execute (`0x005367F0` -> `0x00732280`) before Logic
`0x0055DC9E`. No EventClass is built, and the event table has no selection opcode.
The lock gates the window's key and mouse messages (`0x0054F200`, `0x006930A0`), so
locked input never reaches Select.

Simulation-side callers of `TechnoClass::Select` only pass a selection on: Building
`Mission_Selling` (`0x0044A8C9`, undeploy), Unit `ReceiveDamage` (`0x0073835E`, the
dying transport's passengers and crew), `UnitClass::Deploy` (`0x007399D4`) and the
parasite's release (`0x0062A3A7`, `0x0062A744`). The receiver and parasite reads test
`HouseClass::IsHumanPlayer` (`0x0050B6F0`), which in modes 1 to 5 is
`this == PlayerPtr`, the local house. Every reader found writes only selection, except
event 33: a Tag on a selected object runs its trigger on the selecting client alone.
Neither `TechnoClass::Select` nor `TagClass::ProcessTriggerEvent` (`0x006E53A0`) has a
mode or house gate; `TriggerClass::RegisterEvent` (`0x007264C0`) was not read, so
"natively a network map with such a trigger desyncs" is a *lead*.

Evidence: the bodies and direct callers above were read in Ghidra; the input order and
Select admission are executed in `tools/input_oracle/selection_navigation`. Scripts are
disabled in the shared Ghidra session, so no instruction scan for other `+0x83`
readers was made: "no other gameplay reader" is a *lead*.

Delivered:
- `Command::Select` is deleted, with its frame-ingress arm, its receiver and
  `has_pending_selection_commands`. Frame ingress now carries only SetGameSpeed.
- Owners, split as natively: the IsSelected byte stays on `GameEntity::selection`,
  owned by `Simulation`, whose lifecycle writers are unchanged; the ordered vector is
  `MatchInputState::selection_order` (app). Input commits each mutation at once through
  `SimRuntime::commit_local_selection`, which binds the match's rules and overlays, to
  `Simulation::commit_local_selection` (the former snapshot receiver: omitted members
  are deselected, new ones run Select and event 33 in ledger order). The ledger is then
  reconciled with the committed bits, and the voices follow. For one new member that
  is `0x006FBFA0`'s order; before, the voice preceded event 33.
- `selection_order_pending` is gone: the ledger always reflects committed bits, and
  Guard, Scatter and every other reader see the committed order.
- A selection mutation under the input lock is refused before any effect, in place of
  the old command's refusal. Every key and mouse entry already checks the lock first,
  so this check is defensive. A map-observation profile Select is refused under the
  lock, as before.
- Saves: `SNAPSHOT_VERSION` 368. Older saves are refused by the version gate, as for
  every bump; the saved local view drops its pending flag. A diagnostic `ReplayLog`
  holding `Select` no longer parses. Native recordings never carried selection.
- Map-observation profiles keep their `{"Select": ...}` entries as a profile-only
  payload that commits at the issue boundary. Receipts drop `selection_pending`;
  validation still accepts it in older sealed receipts.
- Selection stays out of the world hash and the sync checksum.

Player-visible: offline play is unchanged except that a selection takes effect when
the input is handled instead of at the next frame's ingress, so its highlight can
appear a frame sooner, as natively.

Residuals:
- Event 33 trigger actions now run between frames. Their Tag outcomes and sounds
  still leave with the next frame's outputs, but a combat light from a death the
  action causes is dropped by the frame-start clear of `combat_light_requests`.
  Trigger: an event-33 Tag whose action kills. Effect: a missing light flash.
  Retail map usage of event 33 is unchecked.
- Event 33 outputs raised between frames wait in the simulation's frame outputs. A
  save made before the next frame drops them, as snapshots drop those buffers, so the
  restored trigger has sprung without its app effects or sounds; and while frames are
  paused they wait for the next frame. Trigger: an event-33 Tag, then a save or a
  pause before the next frame.
- With several new members, VERA runs every member's Select and event 33, then every
  voice; native runs Select, event 33 and the voice per object. Effect: Main RNG voice
  draws reorder against Main draws in event-33 actions, and a member an earlier
  member's action refuses or removes still gets, or loses, its voice. Trigger: an
  event-33 Tag on one of several objects selected together.
- Event 33 effects are local in network play, as natively, and a `ReplayLog` no
  longer reproduces them, so a replay of a map with such a Tag can mismatch its hash.
- The simulation's own Selects (crew, parasite, deploy, undeploy) set the byte
  directly and raise neither event 33 nor VoiceSelect, where native calls
  `TechnoClass::Select`. Unchanged by 2d.
- Input reaching selection during the network modal pump now commits there, where it
  used to wait for an ordinary frame. Native's pump runs no Execute. Unreachable
  until a LAN or WOL session exists.

2e has no selection work left. The network transport must never carry selection.

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

Native order to keep (see [Match bootstrap](#match-bootstrap)):
- The host resolves random side and color before start and ships the result.
- Random start positions are resolved on every machine during load, after seeding.
- Humans become houses in ascending color order, followed by AI, Neutral and Special.
- The load wait drops laggards with reason 1, and the match continues without them.

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

Scope: per-frame send and receive inside `advance_in_game_runtime`. The stage covers
the following native behavior (see [Native reference](#timing-maxahead-and-framesendrate)):
- Ahead-window pacing and the frame-send-rate policy, including the protocol-2 frames
  that run no DoList.
- The PROCESS_TIME `0x21` and host TIMING `0x20` events that retune MaxAhead and FSR.
- The wait-for-peers stall with its reconnecting dialog and the per-mode timeouts.
- Timeout kicks and REMOVEPLAYER `0x23`.
- Opcode `0x13` EXIT routing for local versus remote players, which `net::lockstep`
  leaves to the session owner.

Mode-4-only behavior (ABOUTTOEXIT host migration, statistics, squads) is out of
scope. Native's in-game "Reconnecting" dialog only waits for a peer to catch up;
dropping and rejoining a match is not native and is out of scope.

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
of gamemd's IPX game list and lobby: the GameMode-3 branch of state 0x10 at
`0x0052E35D` and `LANLobby__Run` (`0x005DC350`). Session mode is `Lan`. Native
query, expiry, join and start timings and checks are in
[LAN route and lobby](#lan-route-and-lobby). The protocol is VERA's own; the
player-visible list, expiry and start rules follow native.

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
