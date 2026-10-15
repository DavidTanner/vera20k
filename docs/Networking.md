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

Scope: route live command issue through `SynchronizedCommandQueue`, using
`transfer_single_player` and `dispatch_due_offline`. Delete the direct command path it
replaces. Keep RNG and same-frame ordering identical; the convergence tests and the
stage 0 checksum test guard this.

Done when: skirmish and campaign play through the queue with no behavior change, and
the old path is gone.

Why before transport: the queue's frame-stamp and ahead-window semantics are native
behavior the game must already obey before a second client exists.

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
