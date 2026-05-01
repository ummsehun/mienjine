//! PMX-specific rig metadata for IK and advanced skinning.
//!
//! This module stores PMX-specific bone metadata that doesn't fit into the
//! generic `SceneCpu`/`Node` structures, specifically IK chain definitions.

mod physics_meta;
pub use physics_meta::{
    PmxJointCpu, PmxJointKind, PmxPhysicsMeta, PmxRigidBodyCpu, PmxRigidCalcMethod, PmxRigidShape,
};

mod types;
pub use types::{IKChain, IKLink, PmxBoneMeta, PmxGrantTransform, PmxRigMeta};

mod bone;
pub use bone::{apply_append_bone_transforms, apply_pmx_bone_axis_constraints};

mod ik;
pub use ik::{compute_bone_position, solve_ik_chain_ccd};

#[cfg(test)]
mod tests;
