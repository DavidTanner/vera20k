//! Building controls operate on GameEntity's sole private native stage.
//! The existing construction owner retains Begin_Mode/ready/wrap decisions;
//! this child can borrow that metadata beside the stage without exposing a
//! writable stage view. Native4509DE..450A38 steps here; Techno6FABC2 skips
//! Buildings, preserving their existing scheduler placement.

use super::GameEntity;
use crate::sim::building_construction::{ConstructionFrame, PackUpFrame};
use crate::sim::components::{BuildingDown, BuildingUp, BuildupStage};
use crate::sim::game_options::GameOptions;

impl GameEntity {
    /// Unlimbo's Begin_Mode(0), then the placement route's mission metadata.
    pub(crate) fn install_building_up(&mut self, up: BuildingUp, now: i32) {
        up.anim.restart(&mut self.stage, now);
        self.building_up = Some(up);
    }

    /// Commencing Selling does not restart the stage. Sell's later stage-1
    /// Begin_Mode(0) owns that write (`44A8A2..44A8B5`).
    pub(crate) fn install_building_down(&mut self, down: BuildingDown) {
        self.building_up = None;
        self.building_down = Some(down);
    }

    pub(crate) fn advance_building_up(
        &mut self,
        now: i32,
        options: &GameOptions,
    ) -> ConstructionFrame {
        self.building_up
            .as_mut()
            .expect("construction frame requires a building-up receiver")
            .frame(&mut self.stage, now, options)
    }

    /// The completed construction's queued Begin_Mode(1) applies in Update's
    /// tail (`43FFB4..440042`). The original integrated placement rows retain
    /// F8=0 after this handoff; the earlier mission-only stop retains its last
    /// construction frame. Both controls use the same clock restart owner.
    pub(crate) fn finish_building_up(&mut self, now: i32) {
        BuildupStage::begin(crate::rules::buildup_asset_catalog::NO_BUILDUP)
            .restart(&mut self.stage, now);
        self.building_up = None;
    }

    pub(crate) fn advance_building_down(
        &mut self,
        status: &mut u32,
        now: i32,
        archive_less_sale: bool,
        options: &GameOptions,
    ) -> PackUpFrame {
        self.building_down
            .as_mut()
            .expect("selling frame requires a building-down receiver")
            .frame(&mut self.stage, status, now, archive_less_sale, options)
    }

    pub(crate) fn begin_building_pack_up_stage_two(&mut self, status: &mut u32, now: i32) {
        self.building_down
            .as_mut()
            .expect("Sell stage one requires building-down metadata")
            .begin_stage_two(&mut self.stage, status, now);
    }

    /// Preview the same construction receiver with copies of only its metadata
    /// and clock. No caller writes the retained stage or repeats its arithmetic.
    pub(crate) fn construction_completes_at(&self, now: i32, options: &GameOptions) -> bool {
        self.building_up
            .as_ref()
            .is_some_and(|up| up.completes_at(&self.stage, now, options))
    }

    /// Native +F8, read by BState-0 SHP drawing; this is never a render clock.
    pub fn construction_stage_value(&self) -> i32 {
        self.stage.value()
    }
}
