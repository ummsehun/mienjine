use std::path::Path;

use anyhow::{Result, bail};

pub(super) const SUPPORTED_REQUIRED_EXTENSIONS: &[&str] = &["KHR_texture_transform"];
pub(super) const SUPPORTED_USED_EXTENSIONS: &[&str] = &["KHR_texture_transform"];

pub(crate) fn unsupported_required_extensions(document: &gltf::Document) -> Vec<String> {
    document
        .extensions_required()
        .filter(|ext| !SUPPORTED_REQUIRED_EXTENSIONS.contains(ext))
        .map(ToOwned::to_owned)
        .collect()
}

pub(crate) fn validate_supported_required_extensions(
    document: &gltf::Document,
    path: &Path,
) -> Result<()> {
    let unsupported = unsupported_required_extensions(document);
    if unsupported.is_empty() {
        return Ok(());
    }
    bail!(
        "GLB/glTF requires unsupported extension(s) [{}]: {}",
        unsupported.join(", "),
        path.display()
    );
}

pub(crate) fn unsupported_used_extensions(document: &gltf::Document) -> Vec<String> {
    document
        .extensions_used()
        .filter(|ext| !SUPPORTED_USED_EXTENSIONS.contains(ext))
        .map(ToOwned::to_owned)
        .collect()
}
