use std::path::Path;

use anyhow::Result;

use crate::scene::SceneCpu;

use super::gltf_load::load_gltf_impl;
use super::obj::load_obj_impl;
use super::pmx_load::load_pmx_impl;

pub fn load_obj(path: &Path) -> Result<SceneCpu> {
    load_obj_impl(path)
}

pub fn load_gltf(path: &Path) -> Result<SceneCpu> {
    load_gltf_impl(path)
}

pub fn load_pmx(path: &Path) -> Result<SceneCpu> {
    load_pmx_impl(path)
}
