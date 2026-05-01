use glam::{Mat4, Vec3};

use crate::scene::{MaterialMorphCpu, MaterialMorphFormula, MaterialMorphOp, Node, SdefVertexCpu};

pub(super) fn extract_material_morphs(
    morphs: &[PMXUtil::pmx_types::PMXMorph],
) -> Vec<MaterialMorphCpu> {
    let mut result = Vec::new();

    for morph in morphs {
        let first_data = match morph.morph_data.first() {
            Some(d) => d,
            None => continue,
        };

        if let PMXUtil::pmx_types::MorphTypes::Material(_) = first_data {
            let mut operations = Vec::new();
            for morph_data in &morph.morph_data {
                if let PMXUtil::pmx_types::MorphTypes::Material(mm) = morph_data {
                    let formula = match mm.formula {
                        0 => MaterialMorphFormula::Multiply,
                        _ => MaterialMorphFormula::Add,
                    };
                    operations.push(MaterialMorphOp {
                        target_material_index: mm.index,
                        formula,
                        diffuse: mm.diffuse,
                        specular: mm.specular,
                        specular_factor: mm.specular_factor,
                        ambient: mm.ambient,
                        edge_color: mm.edge_color,
                        edge_size: mm.edge_size,
                        texture_factor: mm.texture_factor,
                        sphere_texture_factor: mm.sphere_texture_factor,
                        toon_texture_factor: mm.toon_texture_factor,
                    });
                }
            }
            if !operations.is_empty() {
                result.push(MaterialMorphCpu {
                    name: morph.name.clone(),
                    operations,
                });
            }
        }
    }

    result
}

pub(super) fn extract_pmx_rig_metadata(
    bones: &[PMXUtil::pmx_types::PMXBone],
) -> crate::engine::pmx_rig::PmxRigMeta {
    use crate::engine::pmx_rig::{IKChain, IKLink, PmxBoneMeta, PmxGrantTransform, PmxRigMeta};

    let bone_meta = bones
        .iter()
        .map(|bone| PmxBoneMeta {
            grant_transform: (bone.append_bone_index >= 0
                && (bone.boneflag & (0x0100 | 0x0200) != 0))
                .then_some(PmxGrantTransform {
                    parent_index: bone.append_bone_index as usize,
                    weight: bone.append_weight,
                    is_local: bone.boneflag & PMXUtil::pmx_types::BONE_FLAG_APPEND_LOCAL_MASK != 0,
                    affects_rotation: bone.boneflag & 0x0100 != 0,
                    affects_translation: bone.boneflag & 0x0200 != 0,
                }),
            name: bone.name.clone(),
            name_en: bone.english_name.clone(),
            position: bone.position.into(),
            parent_index: bone.parent,
            deform_depth: bone.deform_depth,
            boneflag: bone.boneflag,
            offset: bone.offset.into(),
            child: bone.child,
            append_bone_index: bone.append_bone_index,
            append_weight: bone.append_weight,
            fixed_axis: bone.fixed_axis.into(),
            local_axis_x: bone.local_axis_x.into(),
            local_axis_z: bone.local_axis_z.into(),
            key_value: bone.key_value,
            ik_target_index: bone.ik_target_index,
            ik_iter_count: bone.ik_iter_count,
            ik_limit: bone.ik_limit,
        })
        .collect();
    let mut ik_chains = Vec::new();

    for (bone_index, bone) in bones.iter().enumerate() {
        if bone.ik_target_index < 0 {
            continue;
        }

        let target_bone_index = bone.ik_target_index as usize;

        if bone.ik_links.is_empty() {
            continue;
        }

        let chain_root_bone_index = bone
            .ik_links
            .first()
            .map(|l| l.ik_bone_index as usize)
            .unwrap_or(target_bone_index);

        let links: Vec<IKLink> = bone
            .ik_links
            .iter()
            .map(|link| {
                let angle_limits = if link.enable_limit == 1 {
                    Some([
                        Vec3::new(link.limit_min[0], link.limit_min[1], link.limit_min[2]),
                        Vec3::new(link.limit_max[0], link.limit_max[1], link.limit_max[2]),
                    ])
                } else {
                    None
                };
                IKLink {
                    bone_index: link.ik_bone_index as usize,
                    angle_limits,
                }
            })
            .collect();

        ik_chains.push(IKChain {
            controller_bone_index: bone_index,
            target_bone_index,
            chain_root_bone_index,
            iterations: bone.ik_iter_count.max(1) as u32,
            limit_angle: bone.ik_limit.abs().max(0.001),
            links,
        });
    }

    let mut meta = PmxRigMeta {
        bones: bone_meta,
        ik_chains,
        grant_evaluation_order: Vec::new(),
        grant_cycle_bones: Vec::new(),
    };
    meta.rebuild_grant_evaluation_order();
    meta
}

pub(super) fn extract_pmx_physics_metadata(
    rigid_bodies: &[PMXUtil::pmx_types::PMXRigid],
    joints: &[PMXUtil::pmx_types::PMXJoint],
) -> crate::engine::pmx_rig::PmxPhysicsMeta {
    use crate::engine::pmx_rig::{
        PmxJointCpu, PmxJointKind, PmxPhysicsMeta, PmxRigidBodyCpu, PmxRigidCalcMethod,
        PmxRigidShape,
    };

    let rigid_bodies = rigid_bodies
        .iter()
        .map(|rigid| PmxRigidBodyCpu {
            name: rigid.name.clone(),
            name_en: rigid.name_en.clone(),
            bone_index: rigid.bone_index,
            group: rigid.group,
            un_collision_group_flag: rigid.un_collision_group_flag,
            form: match rigid.form {
                PMXUtil::pmx_types::PMXRigidForm::Sphere => PmxRigidShape::Sphere,
                PMXUtil::pmx_types::PMXRigidForm::Box => PmxRigidShape::Box,
                PMXUtil::pmx_types::PMXRigidForm::Capsule => PmxRigidShape::Capsule,
            },
            size: rigid.size.into(),
            position: rigid.position.into(),
            rotation: rigid.rotation.into(),
            mass: rigid.mass,
            move_resist: rigid.move_resist,
            rotation_resist: rigid.rotation_resist,
            repulsion: rigid.repulsion,
            friction: rigid.friction,
            calc_method: match rigid.calc_method {
                PMXUtil::pmx_types::PMXRigidCalcMethod::Static => PmxRigidCalcMethod::Static,
                PMXUtil::pmx_types::PMXRigidCalcMethod::Dynamic => PmxRigidCalcMethod::Dynamic,
                PMXUtil::pmx_types::PMXRigidCalcMethod::DynamicWithBonePosition => {
                    PmxRigidCalcMethod::DynamicWithBonePosition
                }
            },
        })
        .collect();

    let joints = joints
        .iter()
        .map(|joint| PmxJointCpu {
            name: joint.name.clone(),
            name_en: joint.name_en.clone(),
            kind: match &joint.joint_type {
                PMXUtil::pmx_types::PMXJointType::Spring6DOF {
                    a_rigid_index,
                    b_rigid_index,
                    position,
                    rotation,
                    move_limit_down,
                    move_limit_up,
                    rotation_limit_down,
                    rotation_limit_up,
                    spring_const_move,
                    spring_const_rotation,
                } => PmxJointKind::Spring6Dof {
                    a_rigid_index: *a_rigid_index,
                    b_rigid_index: *b_rigid_index,
                    position: (*position).into(),
                    rotation: (*rotation).into(),
                    move_limit_down: (*move_limit_down).into(),
                    move_limit_up: (*move_limit_up).into(),
                    rotation_limit_down: (*rotation_limit_down).into(),
                    rotation_limit_up: (*rotation_limit_up).into(),
                    spring_const_move: (*spring_const_move).into(),
                    spring_const_rotation: (*spring_const_rotation).into(),
                },
                PMXUtil::pmx_types::PMXJointType::_6DOF {
                    a_rigid_index,
                    b_rigid_index,
                    position,
                    rotation,
                    move_limit_down,
                    move_limit_up,
                    rotation_limit_down,
                    rotation_limit_up,
                } => PmxJointKind::SixDof {
                    a_rigid_index: *a_rigid_index,
                    b_rigid_index: *b_rigid_index,
                    position: (*position).into(),
                    rotation: (*rotation).into(),
                    move_limit_down: (*move_limit_down).into(),
                    move_limit_up: (*move_limit_up).into(),
                    rotation_limit_down: (*rotation_limit_down).into(),
                    rotation_limit_up: (*rotation_limit_up).into(),
                },
                PMXUtil::pmx_types::PMXJointType::P2P {
                    a_rigid_index,
                    b_rigid_index,
                    position,
                    rotation,
                } => PmxJointKind::P2P {
                    a_rigid_index: *a_rigid_index,
                    b_rigid_index: *b_rigid_index,
                    position: (*position).into(),
                    rotation: (*rotation).into(),
                },
                PMXUtil::pmx_types::PMXJointType::ConeTwist {
                    a_rigid_index,
                    b_rigid_index,
                    swing_span1,
                    swing_span2,
                    twist_span,
                    softness,
                    bias_factor,
                    relaxation_factor,
                    damping,
                    fix_thresh,
                    enable_motor,
                    max_motor_impulse,
                    motor_target_in_constraint_space,
                } => PmxJointKind::ConeTwist {
                    a_rigid_index: *a_rigid_index,
                    b_rigid_index: *b_rigid_index,
                    swing_span1: *swing_span1,
                    swing_span2: *swing_span2,
                    twist_span: *twist_span,
                    softness: *softness,
                    bias_factor: *bias_factor,
                    relaxation_factor: *relaxation_factor,
                    damping: *damping,
                    fix_thresh: *fix_thresh,
                    enable_motor: *enable_motor,
                    max_motor_impulse: *max_motor_impulse,
                    motor_target_in_constraint_space: (*motor_target_in_constraint_space).into(),
                },
                PMXUtil::pmx_types::PMXJointType::Slider {
                    a_rigid_index,
                    b_rigid_index,
                    lower_linear_limit,
                    upper_linear_limit,
                    lower_angle_limit,
                    upper_angle_limit,
                    power_linear_motor,
                    target_linear_motor_velocity,
                    max_linear_motor_force,
                    power_angler_motor,
                    target_angler_motor_velocity,
                    max_angler_motor_force,
                } => PmxJointKind::Slider {
                    a_rigid_index: *a_rigid_index,
                    b_rigid_index: *b_rigid_index,
                    lower_linear_limit: *lower_linear_limit,
                    upper_linear_limit: *upper_linear_limit,
                    lower_angle_limit: *lower_angle_limit,
                    upper_angle_limit: *upper_angle_limit,
                    power_linear_motor: *power_linear_motor,
                    target_linear_motor_velocity: *target_linear_motor_velocity,
                    max_linear_motor_force: *max_linear_motor_force,
                    power_angler_motor: *power_angler_motor,
                    target_angler_motor_velocity: *target_angler_motor_velocity,
                    max_angler_motor_force: *max_angler_motor_force,
                },
                PMXUtil::pmx_types::PMXJointType::Hinge {
                    a_rigid_index,
                    b_rigid_index,
                    low,
                    high,
                    softness,
                    bias_factor,
                    relaxation_factor,
                    enable_motor,
                    target_velocity,
                    max_motor_impulse,
                } => PmxJointKind::Hinge {
                    a_rigid_index: *a_rigid_index,
                    b_rigid_index: *b_rigid_index,
                    low: *low,
                    high: *high,
                    softness: *softness,
                    bias_factor: *bias_factor,
                    relaxation_factor: *relaxation_factor,
                    enable_motor: *enable_motor,
                    target_velocity: *target_velocity,
                    max_motor_impulse: *max_motor_impulse,
                },
            },
        })
        .collect();

    PmxPhysicsMeta {
        rigid_bodies,
        joints,
    }
}

pub(super) fn convert_vertex_weight(
    weight_type: &PMXUtil::pmx_types::PMXVertexWeight,
) -> ([u16; 4], [f32; 4], Option<SdefVertexCpu>) {
    match weight_type {
        PMXUtil::pmx_types::PMXVertexWeight::BDEF1(idx) => {
            ([*idx as u16, 0, 0, 0], [1.0, 0.0, 0.0, 0.0], None)
        }
        PMXUtil::pmx_types::PMXVertexWeight::BDEF2 {
            bone_index_1,
            bone_index_2,
            bone_weight_1,
        } => (
            [*bone_index_1 as u16, *bone_index_2 as u16, 0, 0],
            [*bone_weight_1, 1.0 - bone_weight_1, 0.0, 0.0],
            None,
        ),
        PMXUtil::pmx_types::PMXVertexWeight::BDEF4 {
            bone_index_1,
            bone_index_2,
            bone_index_3,
            bone_index_4,
            bone_weight_1,
            bone_weight_2,
            bone_weight_3,
            bone_weight_4,
        } => (
            [
                *bone_index_1 as u16,
                *bone_index_2 as u16,
                *bone_index_3 as u16,
                *bone_index_4 as u16,
            ],
            [
                *bone_weight_1,
                *bone_weight_2,
                *bone_weight_3,
                *bone_weight_4,
            ],
            None,
        ),
        PMXUtil::pmx_types::PMXVertexWeight::SDEF {
            bone_index_1,
            bone_index_2,
            bone_weight_1,
            sdef_c,
            sdef_r0,
            sdef_r1,
        } => (
            [*bone_index_1 as u16, *bone_index_2 as u16, 0, 0],
            [*bone_weight_1, 1.0 - bone_weight_1, 0.0, 0.0],
            Some(SdefVertexCpu {
                bone_index_1: *bone_index_1 as u16,
                bone_index_2: *bone_index_2 as u16,
                bone_weight_1: *bone_weight_1,
                c: (*sdef_c).into(),
                r0: (*sdef_r0).into(),
                r1: (*sdef_r1).into(),
            }),
        ),
        PMXUtil::pmx_types::PMXVertexWeight::QDEF {
            bone_index_1,
            bone_index_2,
            bone_index_3,
            bone_index_4,
            bone_weight_1,
            bone_weight_2,
            bone_weight_3,
            bone_weight_4,
        } => (
            [
                *bone_index_1 as u16,
                *bone_index_2 as u16,
                *bone_index_3 as u16,
                *bone_index_4 as u16,
            ],
            [
                *bone_weight_1,
                *bone_weight_2,
                *bone_weight_3,
                *bone_weight_4,
            ],
            None,
        ),
    }
}

pub(super) fn compute_pmx_inverse_bind_mats(nodes: &[Node]) -> Vec<Mat4> {
    let mut bind_globals = vec![Mat4::IDENTITY; nodes.len()];
    let mut visited = vec![false; nodes.len()];
    for index in 0..nodes.len() {
        compute_bind_global(index, nodes, &mut bind_globals, &mut visited);
    }
    bind_globals
        .iter()
        .map(|matrix| matrix.inverse())
        .collect::<Vec<_>>()
}

fn compute_bind_global(
    index: usize,
    nodes: &[Node],
    globals: &mut [Mat4],
    visited: &mut [bool],
) -> Mat4 {
    if visited[index] {
        return globals[index];
    }
    let local = Mat4::from_scale_rotation_translation(
        nodes[index].base_scale,
        nodes[index].base_rotation,
        nodes[index].base_translation,
    );
    let global = if let Some(parent) = nodes[index].parent {
        compute_bind_global(parent, nodes, globals, visited) * local
    } else {
        local
    };
    globals[index] = global;
    visited[index] = true;
    global
}
