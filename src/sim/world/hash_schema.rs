//! Historical projections of the deterministic Rust hash stream.
//!
//! All policies use the current signed health fold (schema 169). The retired
//! cached maximum cannot be reconstructed from live Strength: old arbitrary
//! entity maxima are lost. Thus pre-169 projections exclude selected features,
//! but cannot reproduce old hash streams or certify their golden values.
//! These policies preserve feature-exclusion probes; they do not promise
//! snapshot loading compatibility or native parity for arbitrary versions.
//! Pre-167 projections assume the retired detached track options were absent
//! in the original fixture. Retained active-track evidence rejects that bounded
//! projection: an obsolete cursor/geometry copy cannot be reconstructed from
//! the current authority. Callers must establish the original fixture's state;
//! this policy cannot detect an arbitrary stale copy in an old snapshot.
//! Pre-186 projections omit the pending-ammo byte and assume the removed
//! AircraftReleaseTail was absent and old Attack booleans were both false.
//! Those bounded fixtures are recoverable; arbitrary former aircraft state is not.
//! Pre-187 projections omit the retained burst index and restore a zero remaining-
//! shot byte on AttackTarget. Only the established zero-count fixtures support it.
//! Pre-190 projections omit Foot+55C but cannot recover the former wall-only
//! plane after Foot events mutate it. Historical probes without a retained
//! plane remain comparable; arbitrary pre-190 counter histories do not.
//! No policy folds the retired weapon id of an object's last live selection
//! (`current_weapon_ref`, dropped at snapshot 212): nothing retained records
//! it, so every projection lost that fold at once and the harness pins were
//! re-baselined in that one step. Since then a weapon name is first interned
//! when its shot is emitted (after its warhead), not at selection, so later
//! interned ids can be numbered differently from a pre-212 run: raw-index
//! folds such as a bullet's weapon and warhead move with them, gameplay does
//! not (nothing orders or looks up by weapon ids).
//! Schema217 replaces approximate guided heading/age/phase with the native
//! velocity and signed control fields. Former arbitrary guided states cannot
//! be reconstructed; earlier feature projections do not recover those hashes.
//! No policy folds the retired 8-bit facing mirror, turn target or optional
//! body interpolator (dropped at snapshot 237): nothing retained records the
//! mirror's lag or the target, so every projection folds the body FacingClass
//! in their place and the harness pins were re-baselined in that one step.
//! Schema230's building-facing gate went with them: a building's `+0x388` is
//! its body FacingClass, which every projection folds.

#[derive(Clone, Copy)]
pub(super) enum HashSchema {
    Current,
    /// Bounded full08 diagnostic: old Building slot fold, before power state.
    #[cfg(test)]
    BeforeBuildingPowerIntegration,
    #[cfg(test)]
    Before(u16),
}

/// First hash schema containing each gated layout. A feature can select an
/// alternate encoding, not just append fields: retain both sides of its fold.
#[derive(Clone, Copy)]
#[repr(u16)]
pub(super) enum HashFeature {
    Lifecycle = 28,
    Mission = 29,
    MasterFrame = 43,
    EntityAnimation = 44,
    /// Reserved historical positional bit; obsolete overlay state no longer exists.
    #[cfg(test)]
    RetiredBuildingAnimOverlays = 45,
    TerminalScore = 46,
    PlayfieldAuthority = 47,
    TechnoPlayfield = 87,
    SensorDeposit = 88,
    RealCellBridgeFlags = 90,
    BaseDefenseResponse = 97,
    TechnoConstructor = 104,
    SparkDummyLevelSlope = 107,
    AlternateBaseCenter = 108,
    NavalBuildConst = 109,
    BasePlan = 110,
    BasePlanCenter = 111,
    HouseDeployLatches = 112,
    HouseUpdateActivation = 113,
    CrateAuthority = 114,
    WallRuntime = 115,
    DisguiseDetect = 117,
    HouseHarvesterNoOre = 132,
    HouseEva = 133,
    CreditIncome = 135,
    InfantryTerminal = 136,
    SustainedGapSight = 142,
    GapOperational = 144,
    BridgePublication = 145,
    CellMembership = 159,
    FootPathRuntime = 160,
    BridgeLocomotorAndDummy = 161,
    #[cfg(test)]
    TrackAuthority = 167,
    EstimatedHealth = 168,
    AircraftDockState = 170,
    AnimationAuthority = 171,
    /// Removes folds instead of adding them: the node-era tiberium scanner
    /// state and the fallback ore overlay id no longer exist. Earlier schemas
    /// fold zero/empty/`None` in their place, which is what the pinned harness
    /// fixtures held. A scenario finalized by the map loader held
    /// `Some(first TIB* id)`, so this projection reproduces those fixtures' old
    /// hashes, not an arbitrary pre-174 stream.
    RetiredTiberiumNodeState = 174,
    FootCrateSpeed = 181,
    DisplayLayers = 182,
    #[cfg(test)]
    AnimationDisplay = 184,
    AircraftReleaseAuthority = 186,
    WeaponBurstAuthority = 187,
    FlyDestination = 188,
    TechnoMissionOnly = 189,
    FootNeighborHistory = 190,
    FlyCruiseMode = 191,
    FlyLanding = 192,
    Parasite = 193,
    CrewSurvival = 194,
    MindControl = 196,
    Temporal = 197,
    /// Replaces the two retired per-house counts with `house_tracking`;
    /// earlier schemas recompute the retired counts from the entity store.
    HouseDefeatTracking = 198,
    /// A carried Crazy Ivan bomb (`ObjectClass+0x38` and its `BombClass`).
    IvanBomb = 199,
    /// Gattling stage, value and report latch (`TechnoClass+0x140`, `+0x144`,
    /// `+0x4B8`).
    Gattling = 200,
    /// The object's rearm countdown (`TechnoClass+0x2EC`) replaces the
    /// AttackTarget cooldown/burst-delay counters and the cloak runtime's
    /// copy. Earlier schemas fold zero counters on AttackTarget (the bounded
    /// fixtures had no reload in progress) and the timer in the cloak slot.
    RearmTimer = 202,
    /// A bullet's OnBridge (`ObjectClass+0x8C`, set for an Inviso shot) and
    /// the house ROF bias (`HouseClass+0x1A8`). The bullet no longer keeps
    /// its owner's house (DamageArea reads the live Owner); earlier schemas
    /// fold the live source's house in its place.
    InvisoBullet = 204,
    /// A bullet's `Arcing=` (`BulletTypeClass+0x29B`), which impact
    /// resolution (`0x00468D80`) reads, and a bouncing anim's BounceClass
    /// body (`AnimClass+0x128`).
    BouncingDebris = 205,
    /// Removes the Chrono Miner's retired dock-phase fold (home refinery,
    /// dock-queued byte, dock phase, pivot facing) and adds the Techno+0x1F8
    /// tag. Earlier schemas fold the constructor defaults in their place,
    /// which every miner outside the retired Chrono phases held.
    RetiredRefineryDockPhase = 206,
    /// Mission_Harvest states 0/1 on the native StageClass: removes the
    /// retired target-ore cell and harvest timer (earlier schemas fold their
    /// defaults) and adds the harvester's Unit+0x6D1/+0x6D2 bytes and its
    /// +0xF8 StageClass (value, timer, rate).
    NativeOreField = 207,
    /// The crash latch and its AI edge (`FootClass+0x425`/`+0x426`) and the
    /// Fly fall counter (`FlyLocomotionClass+0x58`). Earlier schemas fold
    /// nothing: no object crashed before, so every latch and counter was zero.
    AircraftCrash = 208,
    /// The SlaveManagerClass on its master (`TechnoClass+0x2D8`), a slave's
    /// SlaveOwner (`+0x2DC`) and Storage replace the retired constructor
    /// slave pool and slave harvester cursor. Earlier schemas fold the pool
    /// from the manager's live slaves; the retired cursor has no counterpart.
    SlaveManager = 209,
    /// Removes folds: the retired VERA copy of the Jumpjet type block in the
    /// common locomotor runtime, which nothing read. Earlier schemas fold the
    /// non-Jumpjet constants every stashed runtime of the pinned fixtures held;
    /// a Jumpjet's own linked values are not reconstructed.
    RetiredJumpjetLegacyBlock = 210,
    /// A building's AI sale byte (`BuildingClass+0x6DC`). Earlier schemas
    /// fold nothing.
    AiSellable = 213,
    /// A building's repair byte (`BuildingClass+0x6E8`) and AI repair byte
    /// (`+0x6CB`); a house's repair delay (`HouseClass+0x1C0`), auto-repair
    /// latch (`+0x245`) and its timer (`+0x280`). Earlier schemas fold
    /// nothing.
    BuildingRepair = 216,
    /// Scenario constructor cursor and retained native Abstract IDs determine
    /// guided Bullet phase independently of Rust handles. This schema also
    /// introduces retained fallback-cell Land for impact animation selection.
    NativeRuntimeIdentity = 217,
    /// Removes the two VERA copies of a factory's rally point: the house's
    /// last rally click and the per-building rally cell. The rally is the
    /// building's ArchiveTarget (`Techno+0x218`), folded with the
    /// base-defence state. Earlier schemas fold the empty copies in their
    /// place; no pinned fixture sets a rally point.
    RetiredRallyCopies = 220,
    /// Foot+68A is a retained byte, even though its sound guard only tests
    /// nonzero. A tagged suffix for nonzero values preserves the former
    /// zero-byte streams; earlier projections omit this byte entirely.
    FootScoldLatch = 224,
    /// Prism forwarding: a building's support count (`BuildingClass+0x664`),
    /// a bullet's damage multiplier (`BulletClass+0x150`) and each House's
    /// building list (House+0x68), each a tagged suffix only when set (a
    /// nonzero count, a multiplier other than Construct's 256, a non-empty
    /// list), so a stream without them is unchanged. Earlier schemas fold none
    /// of them.
    PrismSupport = 225,
    /// Techno+3CD/+3CE retained sinking and sound-edge state, plus the live
    /// House statistics needed to preserve its two loss records through load.
    /// Default-zero states append nothing; earlier schemas omit both additions.
    ShipSinking = 227,
    /// Each House's FactoryPlant list (House+0x140), whose order fixes the
    /// f32 cost-factor fold; a tagged suffix only when the list is non-empty.
    /// Earlier schemas omit it.
    FactoryPlants = 228,
    /// The computer's base building: each House's production mode, building
    /// choice and naval latch (`HouseClass+0x1E4`, `+0x564C`, `+0x1F0`) and
    /// its on-map gatherer count (`+0x158`); each Construction Yard's own
    /// factory and placement-retry timer (`BuildingClass+0x524`, `+0x550`).
    /// Each folds, tagged, only off its constructor value, so a state without
    /// them hashes as earlier schemas, which fold none of them.
    AiBaseBuilding = 232,
    /// The computer's base defenses: each House's on-map force values
    /// (`HouseClass+0x160A8`, `+0x160AC`, `+0x160B0`), folded, tagged, only
    /// when one is not zero, so a state without them hashes as earlier
    /// schemas, which fold none.
    AiBaseDefense = 233,
    /// The computer's strategy tick: each House's Strategy timer
    /// (`HouseClass+0x5634`/`+0x563C`), folded, tagged, only off its
    /// constructor value, so a state without it hashes as earlier schemas,
    /// which fold none.
    AiStrategy = 234,
    /// The computer's teams: each House's team timer (`HouseClass+0x5798`),
    /// trigger-team ratio (`+0x565C`) and unit choices (`+0x5650`..`+0x5658`),
    /// each folded, tagged, only off its constructor value, and its per-type
    /// counts of buildings, infantry and aircraft and its
    /// `ResourceDestination=` count, tagged once any is set; each live team's
    /// creation frame and forming state; each AITrigger's weight record off
    /// its INI starting weight. A state without them hashes as earlier
    /// schemas, which fold none of them.
    AiTeams = 238,
    /// Each Techno's barrel elevation FacingClass (`TechnoClass+0x370`),
    /// folded, tagged, only once an Unlimbo moved it off its constructor
    /// value, so a state without it hashes as earlier schemas, which fold
    /// none.
    BarrelElevation = 239,
}

impl HashSchema {
    pub(super) const fn includes(self, _feature: HashFeature) -> bool {
        match self {
            Self::Current => true,
            #[cfg(test)]
            Self::BeforeBuildingPowerIntegration => !matches!(
                _feature,
                HashFeature::AircraftDockState
                    | HashFeature::AnimationAuthority
                    | HashFeature::RetiredTiberiumNodeState
                    | HashFeature::FootCrateSpeed
                    | HashFeature::DisplayLayers
                    | HashFeature::AnimationDisplay
                    | HashFeature::AircraftReleaseAuthority
                    | HashFeature::WeaponBurstAuthority
                    | HashFeature::FlyDestination
                    | HashFeature::TechnoMissionOnly
                    | HashFeature::FootNeighborHistory
                    | HashFeature::FlyCruiseMode
                    | HashFeature::FlyLanding
                    | HashFeature::Parasite
                    | HashFeature::CrewSurvival
                    | HashFeature::MindControl
                    | HashFeature::Temporal
                    | HashFeature::HouseDefeatTracking
                    | HashFeature::IvanBomb
                    | HashFeature::Gattling
                    | HashFeature::RearmTimer
                    | HashFeature::InvisoBullet
                    | HashFeature::BouncingDebris
                    | HashFeature::RetiredRefineryDockPhase
                    | HashFeature::NativeOreField
                    | HashFeature::AircraftCrash
                    | HashFeature::SlaveManager
                    | HashFeature::RetiredJumpjetLegacyBlock
                    | HashFeature::AiSellable
                    | HashFeature::BuildingRepair
                    | HashFeature::NativeRuntimeIdentity
                    | HashFeature::RetiredRallyCopies
                    | HashFeature::FootScoldLatch
                    | HashFeature::PrismSupport
                    | HashFeature::ShipSinking
                    | HashFeature::FactoryPlants
                    | HashFeature::AiBaseBuilding
                    | HashFeature::AiBaseDefense
                    | HashFeature::AiStrategy
                    | HashFeature::AiTeams
                    | HashFeature::BarrelElevation
            ),
            #[cfg(test)]
            Self::Before(version) => (_feature as u16) < version,
        }
    }

    pub(super) const fn includes_building_power_integration(self) -> bool {
        #[cfg(test)]
        if matches!(self, Self::BeforeBuildingPowerIntegration) {
            return false;
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detached_track_absence_projection_ends_at_schema167() {
        assert!(!HashSchema::Before(167).includes(HashFeature::TrackAuthority));
        assert!(HashSchema::Before(168).includes(HashFeature::TrackAuthority));
        assert!(HashSchema::Current.includes(HashFeature::TrackAuthority));
    }

    #[test]
    fn retired_tiberium_fold_ends_at_schema174() {
        let feature = HashFeature::RetiredTiberiumNodeState;
        assert!(!HashSchema::Before(174).includes(feature));
        assert!(!HashSchema::BeforeBuildingPowerIntegration.includes(feature));
        assert!(HashSchema::Before(175).includes(feature));
        assert!(HashSchema::Current.includes(feature));
    }

    #[test]
    fn historical_policies_preserve_original_positional_masks() {
        // Frozen from the 25-argument calls before this refactor (main c1983f56).
        // Bit positions retain the original parameter order; expected masks
        // are the old call values, independent of the new version predicate.
        let features = [
            HashFeature::Lifecycle,
            HashFeature::Mission,
            HashFeature::MasterFrame,
            HashFeature::EntityAnimation,
            HashFeature::RetiredBuildingAnimOverlays,
            HashFeature::TerminalScore,
            HashFeature::PlayfieldAuthority,
            HashFeature::TechnoPlayfield,
            HashFeature::SensorDeposit,
            HashFeature::RealCellBridgeFlags,
            HashFeature::BaseDefenseResponse,
            HashFeature::TechnoConstructor,
            HashFeature::SparkDummyLevelSlope,
            HashFeature::AlternateBaseCenter,
            HashFeature::NavalBuildConst,
            HashFeature::BasePlan,
            HashFeature::BasePlanCenter,
            HashFeature::HouseDeployLatches,
            HashFeature::HouseUpdateActivation,
            HashFeature::CrateAuthority,
            HashFeature::WallRuntime,
            HashFeature::DisguiseDetect,
            HashFeature::HouseHarvesterNoOre,
            HashFeature::HouseEva,
            HashFeature::CreditIncome,
        ];
        let cases = [
            ("state_hash", HashSchema::Current, 0x01ffffffu32),
            (
                "state_hash_without_mission_v29",
                HashSchema::Before(29),
                0x00000001u32,
            ),
            (
                "state_hash_before_lifecycle_v28_and_mission_v29",
                HashSchema::Before(28),
                0x00000000u32,
            ),
            (
                "state_hash_without_spark_dummy_level_slope_v107",
                HashSchema::Before(107),
                0x00000fffu32,
            ),
            (
                "state_hash_without_naval_build_const_v109",
                HashSchema::Before(109),
                0x00003fffu32,
            ),
            (
                "state_hash_without_base_plan_v110",
                HashSchema::Before(110),
                0x00007fffu32,
            ),
            (
                "state_hash_without_base_plan_center_v111",
                HashSchema::Before(111),
                0x0000ffffu32,
            ),
            (
                "state_hash_without_house_deploy_latches_v112",
                HashSchema::Before(112),
                0x0001ffffu32,
            ),
            (
                "state_hash_without_house_update_activation_v113",
                HashSchema::Before(113),
                0x0003ffffu32,
            ),
            (
                "state_hash_without_crate_authority_v114",
                HashSchema::Before(114),
                0x0007ffffu32,
            ),
            (
                "state_hash_without_disguise_detect_v117",
                HashSchema::Before(117),
                0x001fffffu32,
            ),
            (
                "state_hash_without_house_harvester_no_ore_v132",
                HashSchema::Before(132),
                0x003fffffu32,
            ),
            (
                "state_hash_without_house_eva_v133",
                HashSchema::Before(133),
                0x007fffffu32,
            ),
            (
                "state_hash_without_credit_income_v135",
                HashSchema::Before(135),
                0x00ffffffu32,
            ),
            (
                "state_hash_without_wall_runtime_v115",
                HashSchema::Before(115),
                0x000fffffu32,
            ),
        ];
        for (name, schema, expected) in cases {
            let actual = features
                .iter()
                .enumerate()
                .fold(0u32, |mask, (bit, &feature)| {
                    mask | (u32::from(schema.includes(feature)) << bit)
                });
            assert_eq!(actual, expected, "{name}");
        }
    }
}
