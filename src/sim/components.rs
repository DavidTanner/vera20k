//! Shared data structs used as fields of GameEntity and components.rs.
//!
//! These are plain data types with no behavior. Game logic lives in systems
//! (movement, combat, etc.). The render loop reads GameEntity fields to
//! determine what to draw and where.
//!
//! ## Design notes
//! - Position stores isometric cell coords only. Where an entity is *drawn* is
//!   `render::locomotor_visual`'s business, derived on read — sim/ writes no
//!   screen coordinates.
//!
//! ## Dependency rules
//! - Part of sim/.
//! - sim/ NEVER depends on render/, ui/, audio/, net/.

use crate::sim::intern::InternedId;
use crate::sim::movement::locomotor::MovementLayer;
use crate::util::fixed_math::SimFixed;

/// World position in isometric cell coordinates plus sub-cell lepton offset.
///
/// RA2 uses leptons as its spatial unit (256 leptons = 1 cell). We store the
/// cell coordinate (rx, ry) plus a sub-cell lepton offset (sub_x, sub_y) to
/// get sub-cell precision without overflowing SimFixed on large maps.
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct Position {
    /// Isometric cell X coordinate.
    rx: u16,
    /// Isometric cell Y coordinate.
    ry: u16,
    /// Elevation level (0 = ground). Each level is 15px visual offset.
    z: u8,
    /// Exact native ObjectClass coordinate Z, in signed leptons.
    ///
    /// Ground movement retains ramp height at the native height-write points;
    /// Drive/Ship residual XY interpolation can retain an earlier sampled Z.
    /// TubeMovement also retains its signed interpolation remainder after exit.
    /// Neither coordinate can be reconstructed from the coarse `z` level or
    /// indiscriminately replaced with the current surface on idle ticks.
    /// None is the legacy coarse/independent-altitude representation.
    #[serde(default)]
    exact_z_leptons: Option<i32>,
    /// Sub-cell lepton offset X (0..256). 128 = cell center.
    /// Provides sub-cell precision for smooth movement and accurate range checks.
    sub_x: SimFixed,
    /// Sub-cell lepton offset Y (0..256). 128 = cell center.
    sub_y: SimFixed,
}

impl Position {
    /// Cell (`rx`, `ry`) at level `z`, with the exact Z and the sub-cell offset.
    pub(crate) const fn new(
        rx: u16,
        ry: u16,
        z: u8,
        exact_z_leptons: Option<i32>,
        sub_x: SimFixed,
        sub_y: SimFixed,
    ) -> Self {
        Self {
            rx,
            ry,
            z,
            exact_z_leptons,
            sub_x,
            sub_y,
        }
    }

    /// Isometric cell X coordinate.
    pub(crate) const fn rx(&self) -> u16 {
        self.rx
    }

    /// Store the cell X coordinate.
    pub(crate) fn set_rx(&mut self, rx: u16) {
        self.rx = rx;
    }

    /// Isometric cell Y coordinate.
    pub(crate) const fn ry(&self) -> u16 {
        self.ry
    }

    /// Store the cell Y coordinate.
    pub(crate) fn set_ry(&mut self, ry: u16) {
        self.ry = ry;
    }

    /// Elevation level (0 = ground).
    pub(crate) const fn z(&self) -> u8 {
        self.z
    }

    /// Store the elevation level.
    pub(crate) fn set_z(&mut self, z: u8) {
        self.z = z;
    }

    /// Exact native ObjectClass coordinate Z, in signed leptons; see the field.
    pub(crate) const fn exact_z_leptons(&self) -> Option<i32> {
        self.exact_z_leptons
    }

    /// Store the exact Z.
    pub(crate) fn set_exact_z_leptons(&mut self, exact_z_leptons: Option<i32>) {
        self.exact_z_leptons = exact_z_leptons;
    }

    /// Sub-cell lepton offset X (0..256).
    pub(crate) const fn sub_x(&self) -> SimFixed {
        self.sub_x
    }

    /// Store the sub-cell offset X.
    pub(crate) fn set_sub_x(&mut self, sub_x: SimFixed) {
        self.sub_x = sub_x;
    }

    /// Sub-cell lepton offset Y (0..256).
    pub(crate) const fn sub_y(&self) -> SimFixed {
        self.sub_y
    }

    /// Store the sub-cell offset Y.
    pub(crate) fn set_sub_y(&mut self, sub_y: SimFixed) {
        self.sub_y = sub_y;
    }
}

// MissionCom, MissionLeaf and the private GameEntity body own retained
// construction and sale state.
pub(crate) use crate::sim::building_construction::BuildingDown;
#[cfg(test)]
pub(crate) use crate::sim::building_construction::BuildingUp;

/// Native-like navigation target reference.
///
/// In gamemd this is an `AbstractClass*`. Phase 1 needs cell targets for normal
/// Drive move commands, while the entity variant gives action lines and later
/// destination paths a shared shape without claiming those paths are complete.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum NavTargetRef {
    Cell { rx: u16, ry: u16 },
    Entity { id: u64 },
    Object { id: u64 },
    Building { id: u64 },
}

impl NavTargetRef {
    pub fn cell(rx: u16, ry: u16) -> Self {
        Self::Cell { rx, ry }
    }

    pub fn object(id: u64) -> Self {
        Self::Object { id }
    }

    pub fn building(id: u64) -> Self {
        Self::Building { id }
    }
}

/// FootClass-style owner navigation fields.
///
/// `nav_com` is the owner destination (Foot+0x5A4).
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct NavigationState {
    /// Foot+55C is retained lifecycle history, not current occupancy.
    #[serde(default)]
    pub(crate) neighbor_state: crate::sim::cell_neighbors::FootNeighborState,
    /// Foot timers, retry count and blockage latch survive path retirement.
    #[serde(default)]
    pub(crate) path_runtime: FootPathRuntime,
    /// Foot-owned direction replay and reference cell (+5E0/+558), shared by
    /// every locomotor instance. Retirement must not discard the owner queue.
    /// Native chain tails reload the owner at Drive4B1DF7 / Ship6A143A.
    #[serde(default)]
    pub(crate) path_replay: FootPathQueue,
    /// NavCom's auxiliary slot, cleared whenever NavCom is published or
    /// stopped.
    #[serde(default)]
    nav_com_aux: Option<NavTargetRef>,
    /// `Foot+0x5A4`, NavCom.
    #[serde(default)]
    nav_com: Option<NavTargetRef>,
    /// `Foot+0x5A8`, SuspendedNavCom.
    #[serde(default)]
    suspended_nav_com: Option<NavTargetRef>,
    /// `Foot+0x598`, the NavQueue, oldest first.
    #[serde(default)]
    nav_queue: Vec<NavTargetRef>,
    /// A class-setter call (vt+0x480) that Rust deferred and still owes: a
    /// mission restore's `Assign_Destination(saved, 1)` (`0x004D8F99`) made
    /// where only NavCom could be written. `Simulation::complete_pending_order`
    /// makes it at the object's next Process entry.
    #[serde(default)]
    pending_destination_setter: bool,
    /// FootClass+530, constructor positive-zero. FootUnlimbo4D72EA..4D72F4
    /// copies TechnoType+2F0 only after successful Techno placement, neighbor
    /// publication and the optional AirTracker tail. It outlives routes and
    /// locomotor instances; native getter4DC760 reads this retained double.
    #[serde(default = "default_threat_avoidance_coefficient")]
    threat_avoidance_coefficient: crate::util::native_x87::NativeF64Bits,
}

const fn default_threat_avoidance_coefficient() -> crate::util::native_x87::NativeF64Bits {
    crate::util::native_x87::NativeF64Bits::POSITIVE_ZERO
}

impl Default for NavigationState {
    fn default() -> Self {
        Self::at_frame(0)
    }
}

impl NavigationState {
    /// Foot constructor-owned navigation state, including its frame anchors.
    pub(crate) fn at_frame(frame: u32) -> Self {
        Self {
            neighbor_state: Default::default(),
            path_runtime: FootPathRuntime::at_frame(frame),
            path_replay: Default::default(),
            nav_com_aux: None,
            nav_com: None,
            suspended_nav_com: None,
            nav_queue: Vec::new(),
            pending_destination_setter: false,
            threat_avoidance_coefficient: default_threat_avoidance_coefficient(),
        }
    }

    /// `Foot+0x5A4`, NavCom.
    pub(crate) fn nav_com(&self) -> Option<NavTargetRef> {
        self.nav_com
    }

    /// Store NavCom.
    pub(crate) fn set_nav_com(&mut self, nav_com: Option<NavTargetRef>) {
        self.nav_com = nav_com;
    }

    /// NavCom's auxiliary slot.
    pub(crate) fn nav_com_aux(&self) -> Option<NavTargetRef> {
        self.nav_com_aux
    }

    /// Store NavCom's auxiliary slot.
    pub(crate) fn set_nav_com_aux(&mut self, nav_com_aux: Option<NavTargetRef>) {
        self.nav_com_aux = nav_com_aux;
    }

    /// `Foot+0x5A8`, SuspendedNavCom.
    pub(crate) fn suspended_nav_com(&self) -> Option<NavTargetRef> {
        self.suspended_nav_com
    }

    /// Store SuspendedNavCom.
    pub(crate) fn set_suspended_nav_com(&mut self, suspended_nav_com: Option<NavTargetRef>) {
        self.suspended_nav_com = suspended_nav_com;
    }

    /// `Foot+0x598`, the NavQueue, oldest first.
    pub(crate) fn nav_queue(&self) -> &[NavTargetRef] {
        &self.nav_queue
    }

    /// A fixture's NavQueue.
    #[cfg(test)]
    pub(crate) fn set_nav_queue_for_test(&mut self, nav_queue: Vec<NavTargetRef>) {
        self.nav_queue = nav_queue;
    }

    /// Empty the NavQueue.
    pub(crate) fn clear_nav_queue(&mut self) {
        self.nav_queue.clear();
    }

    /// Take the NavQueue's oldest entry.
    pub(crate) fn pop_nav_queue_front(&mut self) -> Option<NavTargetRef> {
        (!self.nav_queue.is_empty()).then(|| self.nav_queue.remove(0))
    }

    /// Append to a fixture's NavQueue.
    #[cfg(test)]
    pub(crate) fn push_nav_queue_for_test(&mut self, target: NavTargetRef) {
        self.nav_queue.push(target);
    }

    /// Keep only the NavQueue entries `keep` accepts.
    pub(crate) fn retain_nav_queue(&mut self, keep: impl FnMut(&NavTargetRef) -> bool) {
        self.nav_queue.retain(keep);
    }

    /// A deferred class-setter call is owed; see the field.
    pub(crate) fn pending_destination_setter(&self) -> bool {
        self.pending_destination_setter
    }

    /// Store whether a deferred class-setter call is owed.
    pub(crate) fn set_pending_destination_setter(&mut self, pending: bool) {
        self.pending_destination_setter = pending;
    }

    /// Retained Foot+530 input to4DC760. Team AvoidThreats overrides this
    /// value at the getter; it does not mutate the stored coefficient.
    pub(crate) const fn path_threat_coefficient(&self) -> crate::util::native_x87::NativeF64Bits {
        self.threat_avoidance_coefficient
    }

    /// Foot4DC760 reads its TeamType+F2 override without changing Foot+530.
    /// Original reader/getter controls: astar_threat_inputs.{py,json}.
    pub(crate) fn path_threat_coefficient_for_team(
        &self,
        avoids_threats: bool,
    ) -> crate::util::native_x87::NativeF64Bits {
        if avoids_threats {
            crate::util::native_x87::NativeF64Bits::ONE
        } else {
            self.path_threat_coefficient()
        }
    }

    /// Successful FootUnlimbo4D72F4's sole type-to-object copy. Failed early
    /// placement and failed Mark must retain constructor or prior Load state.
    pub(crate) fn retain_threat_avoidance_after_unlimbo(
        &mut self,
        coefficient: crate::util::native_x87::NativeF64Bits,
    ) {
        self.threat_avoidance_coefficient = coefficient;
    }
}

/// Persistent Foot path state. The FootClass constructor 0x004D31E0 anchors
/// both timers at the current frame (0x4D3320, 0x4D335B), stores +64C = 10
/// (0x4D332C) and +6B7 = 0 (0x4D3451). Set_Destination_Internal 0x004D94B0
/// rewrites the timers and latch at 0x4D96C2..0x4D9707 for every accepted
/// setter, including a null destination.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct FootPathRuntime {
    /// Foot+640/+648, including the native frame anchor.
    movement_timer: crate::sim::timer::CdTimer,
    /// Foot+668/+670.
    blocked_timer: crate::sim::timer::CdTimer,
    /// Foot+6B7.
    path_blocked: bool,
    /// Foot+64C, a dword decremented only while nonzero.
    retries_left: u32,
    /// Foot+68A: the pending path-failure sound byte. Ordinary construction
    /// clears it (0x004D33B4); raw Load (0x004103CD) and the no-init Foot
    /// constructor (0x004D3540) preserve the exact byte, including 255.
    /// No ordinary gameplay writer that arms it has been established.
    /// Evidence: `tools/spatial_oracle/foot_scold_latch.{py,json,md}`.
    #[serde(default)]
    scold_latch: u8,
}

impl Default for FootPathRuntime {
    fn default() -> Self {
        Self::at_frame(0)
    }
}

impl FootPathRuntime {
    pub const fn at_frame(frame: u32) -> Self {
        Self {
            movement_timer: crate::sim::timer::CdTimer::started(frame as i32, 0),
            blocked_timer: crate::sim::timer::CdTimer::started(frame as i32, 0),
            path_blocked: false,
            retries_left: 10,
            scold_latch: 0,
        }
    }

    /// Find_Path's urgency on the code-2 ladders (Walk 0x0075B979, Drive/Ship
    /// 0x4B36BC..0x4B36EF): 2 once the latched blockage timer has expired,
    /// else 1.
    pub(crate) fn code2_urgency(&self, frame: u32) -> u8 {
        if self.path_blocked && self.blocked_timer.expired(frame as i32) {
            2
        } else {
            1
        }
    }

    /// `Foot+0x640`/`+0x648`, the movement timer.
    pub(crate) const fn movement_timer(&self) -> crate::sim::timer::CdTimer {
        self.movement_timer
    }

    /// `Foot+0x668`/`+0x670`, the blockage timer.
    pub(crate) const fn blocked_timer(&self) -> crate::sim::timer::CdTimer {
        self.blocked_timer
    }

    /// `Foot+0x6B7`, the blockage latch.
    pub(crate) const fn path_blocked(&self) -> bool {
        self.path_blocked
    }

    /// Store the blockage latch.
    pub(crate) fn set_path_blocked(&mut self, blocked: bool) {
        self.path_blocked = blocked;
    }

    /// `Foot+0x64C`, the path retries left.
    pub(crate) const fn retries_left(&self) -> u32 {
        self.retries_left
    }

    /// Store the path retries left.
    pub(crate) fn set_retries_left(&mut self, retries: u32) {
        self.retries_left = retries;
    }

    /// Spend one path retry.
    pub(crate) fn decrement_retries_left(&mut self) {
        self.retries_left -= 1;
    }

    /// A fixture's movement timer.
    #[cfg(test)]
    pub(crate) fn set_movement_timer_for_test(&mut self, timer: crate::sim::timer::CdTimer) {
        self.movement_timer = timer;
    }

    /// A fixture's blockage timer.
    #[cfg(test)]
    pub(crate) fn set_blocked_timer_for_test(&mut self, timer: crate::sim::timer::CdTimer) {
        self.blocked_timer = timer;
    }

    /// Native Foot checksum 0x004DBCFE reads the byte without normalizing it.
    pub(crate) const fn scold_latch_raw(&self) -> u8 {
        self.scold_latch
    }

    /// Walk 0x0075B085..0x0075B0B0 tests nonzero, then clears unconditionally.
    /// Callers decide whether that branch requests a sound before clearing.
    pub(crate) fn clear_scold_latch(&mut self) -> bool {
        std::mem::replace(&mut self.scold_latch, 0) != 0
    }

    /// Supply native retained inputs without inventing a gameplay arming edge.
    #[cfg(test)]
    pub(crate) fn set_scold_latch_for_test(&mut self, raw: u8) {
        self.scold_latch = raw;
    }

    /// Original Foot timers retain the binary frame; repeated Process calls
    /// must not consume time (Drive4B3607, Ship6A2C56, Walk75B979).
    pub(crate) fn start_movement(&mut self, frame: u32, duration: i32) {
        self.movement_timer = crate::sim::timer::CdTimer::started(frame as i32, duration);
    }

    pub(crate) fn start_blocked(&mut self, frame: u32, duration: i32) {
        self.blocked_timer = crate::sim::timer::CdTimer::started(frame as i32, duration);
    }
}

/// Integer world coordinate triplet used by DriveLocomotion state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct DriveCoord {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

impl DriveCoord {
    const CELL_LEPTONS: i32 = crate::util::lepton::LEPTONS_PER_CELL_I32;
    const CELL_CENTER: i32 = crate::util::lepton::CELL_CENTER_LEPTON_I32;

    pub fn cell(rx: u16, ry: u16, z: i32) -> Self {
        Self {
            x: i32::from(rx) * Self::CELL_LEPTONS + Self::CELL_CENTER,
            y: i32::from(ry) * Self::CELL_LEPTONS + Self::CELL_CENTER,
            z,
        }
    }
}

/// Foot-owned path replay. Native shifts a 24-dword queue on consumption;
/// Rust retains the consumed prefix with an explicit cursor. Consumers must
/// interpret the remaining suffix, not vector emptiness, as the native queue.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct FootPathQueue {
    /// Foot+5E0: octants0..7, explicit tube direction8, native -1 terminator.
    /// Find_Path4D3E98 writes it; Drive/Ship movement consumes the same owner
    /// memory, independently of which locomotor is currently installed.
    #[serde(default)]
    directions: Vec<u8>,
    #[serde(default)]
    cursor: u16,
    /// Foot+558. Fresh acceptance writes this reference (4B4618/6A3C47);
    /// chain consumption preserves it (4B1DF7/6A143A).
    #[serde(default)]
    reference_cell: Option<(i16, i16)>,
}

impl FootPathQueue {
    /// `Foot+0x5E0`, every direction word, consumed prefix included.
    pub(crate) fn directions(&self) -> &[u8] {
        &self.directions
    }

    /// The index of the first unconsumed word.
    pub(crate) fn cursor(&self) -> u16 {
        self.cursor
    }

    /// `Foot+0x558`, the reference cell.
    pub(crate) fn reference_cell(&self) -> Option<(i16, i16)> {
        self.reference_cell
    }

    /// Drop every direction word.
    pub(crate) fn clear_directions(&mut self) {
        self.directions.clear();
    }

    /// Append a direction word.
    pub(crate) fn push_direction(&mut self, direction: u8) {
        self.directions.push(direction);
    }

    /// Overwrite the word at `slot`, when there is one.
    pub(crate) fn set_direction(&mut self, slot: usize, direction: u8) {
        if let Some(word) = self.directions.get_mut(slot) {
            *word = direction;
        }
    }

    /// Store the cursor.
    pub(crate) fn set_cursor(&mut self, cursor: u16) {
        self.cursor = cursor;
    }

    /// Store the reference cell.
    pub(crate) fn set_reference_cell(&mut self, reference_cell: Option<(i16, i16)>) {
        self.reference_cell = reference_cell;
    }

    /// A fixture's direction words.
    #[cfg(test)]
    pub(crate) fn set_directions_for_test(&mut self, directions: Vec<u8>) {
        self.directions = directions;
    }

    /// A fixture's queue.
    #[cfg(test)]
    pub(crate) fn for_test(
        directions: Vec<u8>,
        cursor: u16,
        reference_cell: Option<(i16, i16)>,
    ) -> Self {
        Self {
            directions,
            cursor,
            reference_cell,
        }
    }

    /// `start` followed by the cells `words` step through from it. A tube
    /// word (8) names no adjacent cell and ends the walk.
    fn walk_words(start: (i16, i16), words: &[u8]) -> Vec<(i16, i16)> {
        let mut cell = start;
        words
            .iter()
            .map_while(|&direction| {
                let (dx, dy) =
                    *crate::util::direction::DIRECTION_DELTAS.get(usize::from(direction))?;
                cell = (
                    cell.0.wrapping_add(dx as i16),
                    cell.1.wrapping_add(dy as i16),
                );
                Some(cell)
            })
            .collect()
    }

    /// The cells the remaining words step through from the reference cell
    /// (Foot+558), as the path markers read them. Diagnostics and tests.
    pub(crate) fn remaining_cells(&self) -> Vec<(i16, i16)> {
        self.reference_cell
            .map(|reference| Self::walk_words(reference, self.remaining_directions()))
            .unwrap_or_default()
    }

    /// The reference cell followed by [`Self::remaining_cells`].
    #[cfg(test)]
    pub(crate) fn route_cells(&self) -> Vec<(u16, u16)> {
        self.reference_cell
            .into_iter()
            .chain(self.remaining_cells())
            .map(|(x, y)| (x as u16, y as u16))
            .collect()
    }

    /// `start` followed by the cells every word (consumed or not) steps
    /// through from it: the route one Find_Path installed from `start`, while
    /// the queue still holds that install's words.
    #[cfg(test)]
    pub(crate) fn installed_cells(&self, start: (u16, u16)) -> Vec<(u16, u16)> {
        std::iter::once((start.0 as i16, start.1 as i16))
            .chain(Self::walk_words(
                (start.0 as i16, start.1 as i16),
                &self.directions,
            ))
            .map(|(x, y)| (x as u16, y as u16))
            .collect()
    }

    /// Native queue emptiness tests the unconsumed head, not retained history.
    pub(crate) fn remaining_directions(&self) -> &[u8] {
        let suffix = &self.directions[usize::from(self.cursor).min(self.directions.len())..];
        &suffix[..suffix
            .iter()
            .position(|&direction| direction == u8::MAX)
            .unwrap_or(suffix.len())]
    }

    /// Drive4B224F/Ship6A1899 overwrites the live queue head with -1 after
    /// terminal PerCell2, preserving the backing suffix and reference cell.
    pub(crate) fn clear_live_head(&mut self) {
        let cursor = usize::from(self.cursor).min(self.directions.len());
        if cursor == self.directions.len() {
            self.directions.push(u8::MAX);
        } else {
            self.directions[cursor] = u8::MAX;
        }
    }
}

/// Foot-owned applied speed, shared by every installed locomotor instance.
///
/// SetSpeedFraction4D3710 writes Foot+578/+57C; GetCurrentSpeed4DB1A0 reads
/// that fraction live (`movement::owner_current_speed`); nothing caches its
/// result. Drive4AF540/Ship69EC50 constructors and DriveEND4AF930 do
/// not own or reset it. Keep this outside both class payloads so a synchronous
/// callback can replace a locomotor without replacing the owner's speed.
/// Original executable witnesses: tools/spatial_oracle/foot_speed_owner.json.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct FootSpeedState {
    /// Foot `+0x578`. The Foot constructor zeroes it (`0x004D327F`/`0x004D328B`,
    /// ported by `Default`); after that only `FootClass::SetSpeedFraction @
    /// 0x004D3710` writes it: [`Self::set_speed_fraction`] for a fixed-point
    /// request and [`Self::set_speed_fraction_native_bits`] for a native double.
    applied_fraction: SimFixed,
    /// Foot+580, initialized to exactly 1.0 at4D3292/4D329B. Retain the
    /// native bits: pickup refuses even the immediate neighbors of 1.0.
    /// Every speed query reads it (GetCurrentSpeed multiplies it in). Its only
    /// native writer is the Speed crate (`accept_speed_crate`), reached from
    /// `CellClass::PickupCrate` (`crates::pickup`) when a Foot commits to a
    /// crate cell; saved and hashed (v181).
    crate_multiplier: crate::util::native_x87::NativeF64Bits,
}

impl Default for FootSpeedState {
    fn default() -> Self {
        Self {
            applied_fraction: crate::util::fixed_math::SIM_ZERO,
            crate_multiplier: crate::util::native_x87::NativeF64Bits::ONE,
        }
    }
}

impl FootSpeedState {
    pub(crate) fn crate_multiplier(&self) -> crate::util::native_x87::NativeF64Bits {
        self.crate_multiplier
    }

    /// Foot `+0x578`, the applied speed fraction.
    pub(crate) fn applied_fraction(&self) -> SimFixed {
        self.applied_fraction
    }

    /// `FootClass::SetSpeedFraction @ 0x004D3710` for a fixed-point request:
    /// at least 1 stores 1, at most 0 stores 0, and anything between is
    /// stored as given ([`Self::stored_speed_fraction`]).
    pub(crate) fn set_speed_fraction(&mut self, fraction: SimFixed) {
        self.applied_fraction = fraction.clamp(
            crate::util::fixed_math::SIM_ZERO,
            crate::util::fixed_math::SIM_ONE,
        );
    }

    /// The double `FootClass::SetSpeedFraction @ 0x004D3710` stores at Foot
    /// `+0x578` for a requested one: at least 1.0, +infinity included, stores
    /// 1.0 (`0x004D3714..0x004D3735`); at most 0, negative zero included, or
    /// NaN stores +0 (`0x004D373C..0x004D375D`); anything between is stored
    /// as given. Original executable witnesses, signed zeros, NaNs,
    /// infinities and denormals included:
    /// `tools/spatial_oracle/foot_speed_owner.json`.
    pub(crate) fn stored_speed_fraction(
        requested: crate::util::native_x87::NativeF64Bits,
    ) -> crate::util::native_x87::NativeF64Bits {
        use crate::util::native_x87::NativeF64Bits;
        const INFINITY_BITS: u64 = 0x7ff0_0000_0000_0000;
        let bits = requested.bits();
        if bits >> 63 != 0 || bits == 0 || bits > INFINITY_BITS {
            // A negative value, a zero of either sign, or NaN.
            NativeF64Bits::POSITIVE_ZERO
        } else if bits >= NativeF64Bits::ONE.bits() {
            NativeF64Bits::ONE
        } else {
            requested
        }
    }

    /// `FootClass::SetSpeedFraction @ 0x004D3710` for a locomotor that computes
    /// its fraction as a native double (the Jumpjet's `Process`): the double
    /// [`Self::stored_speed_fraction`] keeps.
    ///
    /// The stored double becomes `SimFixed` by truncation, from its bits, so
    /// the fraction's readers compare against the truncated thresholds
    /// ([`Self::above_tenth`], [`Self::above_eight_tenths`]). A double within
    /// 2^-16 above a threshold would read as not above it. The Jumpjet's speed
    /// is always a whole number k (it steps by the constructor's 2.0 up and
    /// 3.0 down: stock spells `JumpJetAccel=`, which the case-sensitive reader
    /// never sees, and clamps to its integer cap C and 0), so k/C above 0.1 is
    /// at least 0.1 + 1/(10C), outside that window for any C up to 6553, and
    /// above 0.8 for any C up to 13107; k/C exactly 0.1 or 0.8 divides with
    /// truncation to just below it, as native reads it.
    pub(crate) fn set_speed_fraction_native_bits(&mut self, bits: u64) {
        use crate::util::native_x87::NativeF64Bits;
        let stored = Self::stored_speed_fraction(NativeF64Bits::from_bits(bits)).bits();
        self.applied_fraction = if stored == NativeF64Bits::ONE.bits() {
            crate::util::fixed_math::SIM_ONE
        } else {
            // 0 <= value < 1: floor(value * 2^16) from the significand.
            let exponent = ((stored >> 52) & 0x7ff) as i32;
            let mantissa = stored & ((1 << 52) - 1);
            let significand = if exponent == 0 {
                mantissa
            } else {
                mantissa | (1 << 52)
            };
            let shift = 1075 - i64::from(exponent.max(1)) - 16;
            SimFixed::from_bits(if shift >= 64 {
                0
            } else {
                (significand >> shift) as i32
            })
        };
    }

    /// Foot `+0x578` above 0.1 (`0x007E3860`): the Infantry fire error's
    /// moving gate (`0x0051C9B8`) and the sequencer's default arm
    /// (`0x00520D45`). 0.1 lies between `SimFixed` raw 6553 and 6554; the
    /// truncated threshold keeps a fraction that truncated to 6553 below it.
    /// Original boundary witnesses: `tools/spatial_oracle/infantry_fire_speed.json`.
    pub(crate) fn above_tenth(&self) -> bool {
        self.applied_fraction > SimFixed::ONE / SimFixed::from_num(10)
    }

    /// Foot `+0x578` above 0.8 (`0x007EB5C8`): the Jumpjet infantryman's Fly
    /// over Hover (`0x0052123C`). floor(0.8 * 2^16) = 52428.
    pub(crate) fn above_eight_tenths(&self) -> bool {
        self.applied_fraction > SimFixed::from_bits(52_428)
    }

    /// Cell48303A..483072: an already-modified Foot never stacks this effect.
    /// Class/radius eligibility belongs to the pickup effect caller.
    pub(crate) fn accept_speed_crate(
        &mut self,
        multiplier: crate::util::native_x87::NativeF64Bits,
    ) -> bool {
        use crate::util::native_x87::{MaskedX87Chop53 as X, NativeF64Bits};
        if self.crate_multiplier != NativeF64Bits::ONE {
            return false;
        }
        self.crate_multiplier = X::store_f64_masked_chop(X::mul(
            X::load_f64(self.crate_multiplier),
            X::load_f64(multiplier),
        ));
        true
    }
}

/// One active Drive/Ship locomotor's retained track selector, signed cursor,
/// short-track choice and residual (+58/+5C/+60/+4C). Curve geometry and a
/// temporary Process_Track call must not own serialized copies of this state.
/// Native evidence: tools/spatial_oracle/locomotor_track_cursor.json.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct TrackProgress {
    turn_index: i32,
    /// Next-to-consume cursor. Drive constructor4AF5A6/4AF5A9 and Ship
    /// constructor69ECB6/69ECB9 initialize selector/cursor to -1;
    /// fresh/forced acceptance and completion retirement instead store zero.
    cursor: i32,
    reversed: bool,
    residual: i32,
}

impl TrackProgress {
    /// A fixture's progress.
    #[cfg(test)]
    pub(crate) const fn for_test(
        turn_index: i32,
        cursor: i32,
        reversed: bool,
        residual: i32,
    ) -> Self {
        Self {
            turn_index,
            cursor,
            reversed,
            residual,
        }
    }

    /// See the field.
    pub(crate) const fn turn_index(&self) -> i32 {
        self.turn_index
    }

    /// Store the field.
    pub(crate) fn set_turn_index(&mut self, turn_index: i32) {
        self.turn_index = turn_index;
    }

    /// See the field.
    pub(crate) const fn cursor(&self) -> i32 {
        self.cursor
    }

    /// Store the field.
    pub(crate) fn set_cursor(&mut self, cursor: i32) {
        self.cursor = cursor;
    }

    /// See the field.
    pub(crate) const fn reversed(&self) -> bool {
        self.reversed
    }

    /// Store the field.
    pub(crate) fn set_reversed(&mut self, reversed: bool) {
        self.reversed = reversed;
    }

    /// See the field.
    pub(crate) const fn residual(&self) -> i32 {
        self.residual
    }

    /// Store the field.
    pub(crate) fn set_residual(&mut self, residual: i32) {
        self.residual = residual;
    }
}

impl Default for TrackProgress {
    fn default() -> Self {
        Self {
            turn_index: -1,
            cursor: -1,
            reversed: false,
            residual: 0,
        }
    }
}

/// Drive-owned occupation mark installed ahead of the live object-list cell.
///
/// The cell list remains tied to the unit's committed coordinates. This record
/// persists the independent head-to mark so the transient per-cell occupation
/// index can be rebuilt after loading a snapshot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct DriveOccupationFootprint {
    pub rx: u16,
    pub ry: u16,
    pub layer: MovementLayer,
}

/// Which part of a multi-part voxel model an entity/atlas entry represents.
///
/// Non-turret units use `Composite` (body+turret+barrel baked together).
/// Turret units store Body/Turret/Barrel separately so the turret can
/// be drawn at a different facing than the body at render time.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
pub enum VxlLayer {
    /// All parts composited into one sprite (for units without independent turret).
    Composite,
    /// Body only (hull/chassis).
    Body,
    /// Turret only ({IMAGE}TUR.VXL).
    Turret,
    /// Barrel only ({IMAGE}BARL.VXL).
    Barrel,
    /// Ground shadow of the main voxel: each section's occupied (x, y) columns
    /// flattened onto z = 0 and shifted by the shadow light vector, always at
    /// motion frame 0. See `render::vxl_raster::render_vxl_shadow`.
    Shadow,
}

/// Constructor row for a generic AnimClass-like runtime spawn.
///
/// This preserves the fields passed to `AnimClass::Constructor` separately from
/// presentation conveniences such as cached frame count and wall-clock frame
/// delay, so parity-sensitive code can inspect the original constructor surface.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct AnimClassSpawnDescriptor {
    /// AnimType/SHP type interned ID.
    pub type_name: InternedId,
    /// World cell containing the constructor coordinate.
    pub rx: u16,
    pub ry: u16,
    /// Sub-cell constructor coordinate in leptons.
    pub sub_x: SimFixed,
    pub sub_y: SimFixed,
    /// Height level for the constructor coordinate.
    pub z: u8,
    /// Constructor `delay` argument, in native logic frames.
    pub delay: u16,
    /// Signed constructor `loop` argument.
    pub loop_count: i32,
    /// Constructor draw flags argument.
    pub draw_flags: u32,
    /// Constructor `ZAdjust` argument.
    pub z_adjust: i32,
    /// Constructor reverse argument.
    pub reverse: bool,
    /// AnimClass `+0x196`: draw through the owning cell's palette/light path.
    #[serde(default)]
    pub use_cell_drawer: bool,
    /// AnimClass `+0x197`: marks the instance as terrain-attached.
    #[serde(default)]
    pub terrain_attached: bool,
    /// Instance draw-state bytes supplied by the native producer.
    pub draw_runtime: crate::sim::anim_class::AnimDrawRuntime,
}

impl AnimClassSpawnDescriptor {
    pub fn new(
        type_name: InternedId,
        rx: u16,
        ry: u16,
        sub_x: SimFixed,
        sub_y: SimFixed,
        z: u8,
    ) -> Self {
        Self {
            type_name,
            rx,
            ry,
            sub_x,
            sub_y,
            z,
            delay: 0,
            loop_count: 1,
            draw_flags: 0,
            z_adjust: 0,
            reverse: false,
            use_cell_drawer: false,
            terrain_attached: false,
            draw_runtime: crate::sim::anim_class::AnimDrawRuntime::default(),
        }
    }
}

/// Emitted by the refinery dock state machine for EVERY due dump gate of
/// `UnitClass::Mission_Unload @ 0x0073D630` state 3 (`HarvesterDumpRate × 900
/// <= unit+0xF8`, `0x0073E355..0x0073E374`), including the final gate that finds
/// no cargo. The authoritative master-frame tail consumes it to spawn the
/// refinery smoke burst (`BuildingClass` vtable+0x468 → `0x00459900`, fired
/// first on each gate at `0x0073E37E`), start SpecialAnim slot 10 only while
/// none is live (`building+0x584 == NULL`, `0x0073E384`), and cut the running
/// SpecialAnim on the empty gate (`ClearAnimSlot(0xA)`, `0x0073E530`) before
/// the returned state hash. The queue itself is transient and serde-skipped.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BaleDepositEvent {
    /// Refinery stable_id where the bale was deposited.
    pub building_id: u64,
    /// Sim tick when this event was emitted (for ordering / debugging).
    pub tick: u64,
    /// One StorageClass slot drained on this gate (credits were paid;
    /// `0x0073E4A2..0x0073E4DA`).
    pub drained: bool,
    /// The gate found no cargo (`FindFirstNonEmptySlot == -1`,
    /// `0x0073E4DC..0x0073E534`): state 3 → 4 and the SpecialAnim is cut.
    pub empty: bool,
}

/// Emitted by the tank-bunker lifecycle when the walls rise (install) or fall
/// (teardown). The authoritative frame tail creates the bunker's SpecialAnim
/// overlays — document order within `kind == Special` decides the pair: 0/1 =
/// walls-up, 2/3 = walls-down. `damaged` selects the `…Damaged` art variant when
/// the building was at/below ConditionRed health at emit time. The queue is
/// transient; its resulting overlay component participates in the frame hash.
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct BunkerWallAnimEvent {
    /// Bunker building stable_id whose walls are animating.
    pub building_id: u64,
    /// `true` = walls rising (install), `false` = walls falling (teardown).
    pub up: bool,
    /// Building was at/below ConditionRed when the event fired — use the
    /// `…Damaged` SpecialAnim variant.
    pub damaged: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::movement::DriveLocomotionRuntime;
    use crate::util::fixed_math::SIM_ZERO;

    #[test]
    fn test_position_creation() {
        let pos: Position = Position::new(
            30,
            40,
            0,
            None,
            crate::util::lepton::CELL_CENTER_LEPTON,
            crate::util::lepton::CELL_CENTER_LEPTON,
        );
        assert_eq!(pos.rx, 30);
        assert_eq!(pos.ry, 40);
    }

    #[test]
    fn test_types_are_send_sync() {
        // GameEntity fields must be Send + Sync for future multithreaded sim ticks.
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<Position>();
        assert_send_sync::<crate::sim::combat::Health>();
        assert_send_sync::<BuildingUp>();
        assert_send_sync::<crate::sim::animation::VoxelAnimation>();
        assert_send_sync::<crate::sim::animation::HarvestOverlay>();
        assert_send_sync::<crate::sim::movement::locomotor::LocomotorState>();
        assert_send_sync::<NavigationState>();
        assert_send_sync::<DriveLocomotionRuntime>();
    }

    #[test]
    fn drive_coord_cell_uses_center_leptons() {
        let coord = DriveCoord::cell(45, 40, 0);
        assert_eq!(coord.x, 45 * 256 + 128);
        assert_eq!(coord.y, 40 * 256 + 128);
        assert_eq!(coord.z, 0);
    }

    #[test]
    fn nav_target_ref_has_cell_object_and_building_shapes() {
        assert_eq!(
            NavTargetRef::cell(45, 40),
            NavTargetRef::Cell { rx: 45, ry: 40 }
        );
        assert_eq!(NavTargetRef::object(7), NavTargetRef::Object { id: 7 });
        assert_eq!(NavTargetRef::building(9), NavTargetRef::Building { id: 9 });
    }

    #[test]
    fn drive_locomotion_default_is_inert() {
        let drive = DriveLocomotionRuntime::default();
        assert_eq!(drive.destination(), None);
        assert_eq!(drive.head_to(), None);
        let navigation = NavigationState::default();
        assert!(navigation.path_replay.directions.is_empty());
        assert_eq!(navigation.path_replay.cursor, 0);
        assert_eq!(drive.track().turn_index, -1);
        assert_eq!(drive.track().cursor, -1);
        assert!(!drive.track_valid());
        assert!(!drive.track().reversed);
        assert_eq!(drive.target_speed_fraction(), SIM_ZERO);
        let owner_speed = FootSpeedState::default();
        assert_eq!(owner_speed.applied_fraction, SIM_ZERO);
        assert_eq!(drive.track().residual, 0);
    }

    /// `FootClass::SetSpeedFraction @ 0x004D3710` executed on the original
    /// bytes: the double it stores for each requested one, signed zeros,
    /// NaNs, infinities and denormals included, and that double's truncation
    /// to `SimFixed`.
    #[test]
    fn speed_fraction_setter_matches_the_original() {
        use crate::util::native_x87::NativeF64Bits;
        let cases: Vec<serde_json::Value> = serde_json::from_str(crate::test_fixture::text(
            "tools/spatial_oracle/foot_speed_owner.json",
        ))
        .unwrap();
        let bits = |row: &serde_json::Value, hex: &str, float: &str| {
            row[hex].as_str().map_or_else(
                || row[float].as_f64().unwrap().to_bits(),
                |hex| u64::from_str_radix(hex, 16).unwrap(),
            )
        };
        let mut setter_rows = 0;
        for case in &cases {
            let requested = bits(&case["input"], "requested_bits", "requested");
            let stored = bits(&case["output"], "applied_bits", "applied");
            let requested_double = NativeF64Bits::from_bits(requested);
            assert_eq!(
                FootSpeedState::stored_speed_fraction(requested_double).bits(),
                stored,
                "{case}"
            );
            let mut speed = FootSpeedState::default();
            speed.set_speed_fraction_native_bits(requested);
            // The stored double lies in [0, 1], so scaling by 2^16 is exact.
            let truncated = SimFixed::from_bits((f64::from_bits(stored) * 65536.0) as i32);
            assert_eq!(speed.applied_fraction(), truncated, "{case}");
            setter_rows += usize::from(case["input"]["family"] == "setter");
        }
        assert_eq!((cases.len(), setter_rows), (25, 13));
    }

    #[test]
    fn drive_locomotion_serde_defaults_missing_fields() {
        let drive: DriveLocomotionRuntime = serde_json::from_str("{}").expect("deserialize drive");
        assert_eq!(drive, DriveLocomotionRuntime::default());
    }

    #[test]
    fn drive_locomotion_hash_changes_when_runtime_state_changes() {
        use std::hash::{Hash, Hasher};

        fn hash_drive(drive: &DriveLocomotionRuntime) -> u64 {
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            drive.hash(&mut hasher);
            hasher.finish()
        }

        let drive_a = DriveLocomotionRuntime::default();
        let drive_b = DriveLocomotionRuntime::default()
            .with_destination_for_test(Some(DriveCoord::cell(45, 40, 0)))
            .with_track_for_test(TrackProgress {
                residual: 6,
                ..Default::default()
            });

        assert_ne!(hash_drive(&drive_a), hash_drive(&drive_b));
    }
}
