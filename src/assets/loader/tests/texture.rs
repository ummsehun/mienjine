use tempfile::tempdir;

use super::super::pmx_load::resolve_pmx_texture_path;
use super::super::texture_utils::{fallback_white_texture, load_pmx_texture};

#[test]
fn fallback_white_texture_has_explicit_label() {
    let texture = fallback_white_texture();
    assert_eq!(texture.source_format, "FallbackWhite");
    assert_eq!(texture.width, 1);
    assert_eq!(texture.height, 1);
    assert_eq!(texture.rgba8, vec![255, 255, 255, 255]);
}

#[test]
fn fallback_white_texture_for_missing_pmx_texture() {
    let dir = tempdir().expect("tempdir");
    let fake_path = dir.path().join("nonexistent.png");
    let texture = load_pmx_texture(&fake_path, "missing", 0);
    assert_eq!(texture.source_format, "FallbackWhite");
    assert_eq!(texture.width, 1);
    assert_eq!(texture.height, 1);
    assert_eq!(texture.rgba8, vec![255, 255, 255, 255]);
}

#[test]
fn resolve_pmx_texture_path_normalizes_backslashes_and_tex_prefix() {
    let dir = tempdir().expect("tempdir");
    let model_dir = dir.path().join("miku");
    std::fs::create_dir_all(model_dir.join("tex")).expect("create tex dir");
    let model_path = model_dir.join("rabbit.pmx");
    std::fs::write(&model_path, b"pmx").expect("create pmx file");
    let texture_path = model_dir.join("tex").join("face_Mikuv4x.tga");
    std::fs::write(&texture_path, b"texture").expect("create texture file");

    let resolved = resolve_pmx_texture_path(&model_path, "tex\\face_Mikuv4x.tga");
    assert_eq!(resolved, texture_path);
}

#[test]
fn load_pmx_texture_supports_bmp_and_tga_formats() {
    let dir = tempdir().expect("tempdir");
    let rgba = [11_u8, 22, 33, 255];
    let bmp_path = dir.path().join("test.bmp");
    let tga_path = dir.path().join("test.tga");

    image::save_buffer_with_format(
        &bmp_path,
        &rgba,
        1,
        1,
        image::ColorType::Rgba8,
        image::ImageFormat::Bmp,
    )
    .expect("write bmp");
    image::save_buffer_with_format(
        &tga_path,
        &rgba,
        1,
        1,
        image::ColorType::Rgba8,
        image::ImageFormat::Tga,
    )
    .expect("write tga");

    let bmp = load_pmx_texture(&bmp_path, "bmp", 0);
    let tga = load_pmx_texture(&tga_path, "tga", 0);

    assert_eq!(bmp.width, 1);
    assert_eq!(bmp.height, 1);
    assert_eq!(bmp.rgba8, rgba);
    assert_eq!(tga.width, 1);
    assert_eq!(tga.height, 1);
    assert_eq!(tga.rgba8, rgba);
}
