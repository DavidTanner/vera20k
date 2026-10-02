//! Techno deployment attachment and Unit state access. Native identities and
//! executable controls: tools/spatial_oracle/unit_simple_deploy.{py,json,md}.

use super::GameEntity;
use crate::sim::anim_class::AnimId;
use std::hash::{Hash, Hasher};

impl GameEntity {
    /// Unit746DB0: either transition byte, independent of deployed6E0.
    pub fn unit_deploying(&self) -> bool {
        self.mission_leaf.as_unit().is_some_and(|leaf| {
            leaf.deploy_begin_active() != 0 || leaf.deploy_reverse_active() != 0
        })
    }

    pub(crate) fn deploy_anim(&self) -> Option<AnimId> {
        self.deploy_anim
    }

    ///739BDC/739DC0 retain the constructed Anim until pointer expiry.
    pub(crate) fn retain_deploy_anim(&mut self, anim: AnimId) {
        self.deploy_anim = Some(anim);
    }

    /// Techno710410 (710443..71044E) /70794C clear only the matching pointer.
    pub(crate) fn expire_deploy_anim(&mut self, anim: AnimId) {
        if self.deploy_anim == Some(anim) {
            self.deploy_anim = None;
        }
    }

    /// Techno destructor6F467F takes the retained animation for UnInit.
    pub(crate) fn take_deploy_anim(&mut self) -> Option<AnimId> {
        self.deploy_anim.take()
    }

    pub(crate) fn landing_for_deploy(&self) -> bool {
        self.landing_for_deploy
    }

    ///739B2C requests landing; Jumpjet54CA75/54B46D clear the same byte.
    pub(crate) fn set_landing_for_deploy(&mut self, landing: bool) {
        self.landing_for_deploy = landing;
    }

    pub(crate) fn hash_deploy_attachment(&self, hasher: &mut impl Hasher) {
        self.deploy_anim.hash(hasher);
        self.landing_for_deploy.hash(hasher);
    }

    #[cfg(test)]
    pub(crate) fn set_unit_simple_deploy_for_test(
        &mut self,
        deployed: bool,
        begin: bool,
        reverse: bool,
    ) {
        self.mission_leaf.set_unit_deployed(u8::from(deployed));
        self.mission_leaf
            .set_unit_deploy_begin_active(u8::from(begin));
        self.mission_leaf
            .set_unit_deploy_reverse_active(u8::from(reverse));
    }
}
