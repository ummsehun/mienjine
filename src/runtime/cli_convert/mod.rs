use crate::runtime::cli::{
    AnsiQuantizationArg, AudioReactiveArg, BackendArg, CameraAlignPresetArg, CameraFocusArg,
    CameraModeArg, CellAspectModeArg, CenterLockModeArg, CinematicCameraArg, ClarityProfileArg,
    ColorModeArg, ContrastProfileArg, DetailProfileArg, GraphicsProtocolArg, KittyCompressionArg,
    KittyInternalResArg, KittyPipelineArg, KittyTransportArg, ModeArg, OutputModeArg,
    PerfProfileArg, RecoverColorArg, RecoverStrategyArg, StageRoleArg, SyncPolicyArg,
    SyncProfileModeArg, SyncSpeedModeArg, TextureSamplerModeArg, TextureSamplingArg,
    TextureVOriginArg, ThemeArg, ToggleArg, WasdModeArg,
};
use crate::runtime::sync_profile::SyncProfileMode;
use crate::scene::{
    AnsiQuantization, AudioReactiveMode, BrailleProfile, CameraAlignPreset, CameraControlMode,
    CameraFocusMode, CameraMode, CellAspectMode, CenterLockMode, CinematicCameraMode,
    ClarityProfile, ColorMode, ContrastProfile, DetailProfile, GraphicsProtocol, KittyCompression,
    KittyInternalResPreset, KittyPipelineMode, KittyTransport, PerfProfile, RecoverStrategy,
    RenderBackend, RenderMode, RenderOutputMode, StageRole, SyncPolicy, SyncSpeedMode,
    TextureSamplerMode, TextureSamplingMode, TextureVOrigin, ThemeStyle,
};

impl From<ModeArg> for RenderMode {
    fn from(value: ModeArg) -> Self {
        match value {
            ModeArg::Ascii => RenderMode::Ascii,
            ModeArg::Braille => RenderMode::Braille,
        }
    }
}

impl From<CellAspectModeArg> for CellAspectMode {
    fn from(value: CellAspectModeArg) -> Self {
        match value {
            CellAspectModeArg::Auto => CellAspectMode::Auto,
            CellAspectModeArg::Manual => CellAspectMode::Manual,
        }
    }
}

impl From<ContrastProfileArg> for ContrastProfile {
    fn from(value: ContrastProfileArg) -> Self {
        match value {
            ContrastProfileArg::Adaptive => ContrastProfile::Adaptive,
            ContrastProfileArg::Fixed => ContrastProfile::Fixed,
        }
    }
}

impl From<SyncSpeedModeArg> for SyncSpeedMode {
    fn from(value: SyncSpeedModeArg) -> Self {
        match value {
            SyncSpeedModeArg::Auto => SyncSpeedMode::AutoDurationFit,
            SyncSpeedModeArg::Realtime => SyncSpeedMode::Realtime1x,
        }
    }
}

impl From<SyncPolicyArg> for SyncPolicy {
    fn from(value: SyncPolicyArg) -> Self {
        match value {
            SyncPolicyArg::Continuous => SyncPolicy::Continuous,
            SyncPolicyArg::Fixed => SyncPolicy::Fixed,
            SyncPolicyArg::Manual => SyncPolicy::Manual,
        }
    }
}

impl From<SyncProfileModeArg> for SyncProfileMode {
    fn from(value: SyncProfileModeArg) -> Self {
        match value {
            SyncProfileModeArg::Auto => SyncProfileMode::Auto,
            SyncProfileModeArg::Off => SyncProfileMode::Off,
            SyncProfileModeArg::Write => SyncProfileMode::Write,
        }
    }
}

impl From<OutputModeArg> for RenderOutputMode {
    fn from(value: OutputModeArg) -> Self {
        match value {
            OutputModeArg::Text => RenderOutputMode::Text,
            OutputModeArg::Hybrid => RenderOutputMode::Hybrid,
            OutputModeArg::KittyHq => RenderOutputMode::KittyHq,
        }
    }
}

impl From<KittyTransportArg> for KittyTransport {
    fn from(value: KittyTransportArg) -> Self {
        match value {
            KittyTransportArg::Shm => KittyTransport::Shm,
            KittyTransportArg::Direct => KittyTransport::Direct,
        }
    }
}

impl From<KittyCompressionArg> for KittyCompression {
    fn from(value: KittyCompressionArg) -> Self {
        match value {
            KittyCompressionArg::None => KittyCompression::None,
            KittyCompressionArg::Zlib => KittyCompression::Zlib,
        }
    }
}

impl From<KittyInternalResArg> for KittyInternalResPreset {
    fn from(value: KittyInternalResArg) -> Self {
        match value {
            KittyInternalResArg::R640x360 => KittyInternalResPreset::R640x360,
            KittyInternalResArg::R854x480 => KittyInternalResPreset::R854x480,
            KittyInternalResArg::R1280x720 => KittyInternalResPreset::R1280x720,
        }
    }
}

impl From<StageRoleArg> for StageRole {
    fn from(value: StageRoleArg) -> Self {
        match value {
            StageRoleArg::Sub => StageRole::Sub,
            StageRoleArg::Off => StageRole::Off,
        }
    }
}

impl From<KittyPipelineArg> for KittyPipelineMode {
    fn from(value: KittyPipelineArg) -> Self {
        match value {
            KittyPipelineArg::RealPixel => KittyPipelineMode::RealPixel,
            KittyPipelineArg::GlyphCompat => KittyPipelineMode::GlyphCompat,
        }
    }
}

impl From<RecoverStrategyArg> for RecoverStrategy {
    fn from(value: RecoverStrategyArg) -> Self {
        match value {
            RecoverStrategyArg::Hard => RecoverStrategy::Hard,
            RecoverStrategyArg::Soft => RecoverStrategy::Soft,
            RecoverStrategyArg::Off => RecoverStrategy::Off,
        }
    }
}

impl From<RecoverColorArg> for bool {
    fn from(value: RecoverColorArg) -> Self {
        matches!(value, RecoverColorArg::Auto)
    }
}

impl From<GraphicsProtocolArg> for GraphicsProtocol {
    fn from(value: GraphicsProtocolArg) -> Self {
        match value {
            GraphicsProtocolArg::Auto => GraphicsProtocol::Auto,
            GraphicsProtocolArg::Kitty => GraphicsProtocol::Kitty,
            GraphicsProtocolArg::Iterm2 => GraphicsProtocol::Iterm2,
            GraphicsProtocolArg::None => GraphicsProtocol::None,
        }
    }
}

impl From<ColorModeArg> for ColorMode {
    fn from(value: ColorModeArg) -> Self {
        match value {
            ColorModeArg::Mono => ColorMode::Mono,
            ColorModeArg::Ansi => ColorMode::Ansi,
        }
    }
}

impl From<crate::runtime::cli::BrailleProfileArg> for BrailleProfile {
    fn from(value: crate::runtime::cli::BrailleProfileArg) -> Self {
        match value {
            crate::runtime::cli::BrailleProfileArg::Safe => BrailleProfile::Safe,
            crate::runtime::cli::BrailleProfileArg::Normal => BrailleProfile::Normal,
            crate::runtime::cli::BrailleProfileArg::Dense => BrailleProfile::Dense,
        }
    }
}

impl From<ThemeArg> for ThemeStyle {
    fn from(value: ThemeArg) -> Self {
        match value {
            ThemeArg::Theater => ThemeStyle::Theater,
            ThemeArg::Neon => ThemeStyle::Neon,
            ThemeArg::Holo => ThemeStyle::Holo,
        }
    }
}

impl From<PerfProfileArg> for PerfProfile {
    fn from(value: PerfProfileArg) -> Self {
        match value {
            PerfProfileArg::Balanced => PerfProfile::Balanced,
            PerfProfileArg::Cinematic => PerfProfile::Cinematic,
            PerfProfileArg::Smooth => PerfProfile::Smooth,
        }
    }
}

impl From<DetailProfileArg> for DetailProfile {
    fn from(value: DetailProfileArg) -> Self {
        match value {
            DetailProfileArg::Perf => DetailProfile::Perf,
            DetailProfileArg::Balanced => DetailProfile::Balanced,
            DetailProfileArg::Ultra => DetailProfile::Ultra,
        }
    }
}

impl From<ClarityProfileArg> for ClarityProfile {
    fn from(value: ClarityProfileArg) -> Self {
        match value {
            ClarityProfileArg::Balanced => ClarityProfile::Balanced,
            ClarityProfileArg::Sharp => ClarityProfile::Sharp,
            ClarityProfileArg::Extreme => ClarityProfile::Extreme,
        }
    }
}

impl From<AnsiQuantizationArg> for AnsiQuantization {
    fn from(value: AnsiQuantizationArg) -> Self {
        match value {
            AnsiQuantizationArg::Q216 => AnsiQuantization::Q216,
            AnsiQuantizationArg::Off => AnsiQuantization::Off,
        }
    }
}

impl From<CenterLockModeArg> for CenterLockMode {
    fn from(value: CenterLockModeArg) -> Self {
        match value {
            CenterLockModeArg::Root => CenterLockMode::Root,
            CenterLockModeArg::Mixed => CenterLockMode::Mixed,
        }
    }
}

impl From<CameraFocusArg> for CameraFocusMode {
    fn from(value: CameraFocusArg) -> Self {
        match value {
            CameraFocusArg::Auto => CameraFocusMode::Auto,
            CameraFocusArg::Full => CameraFocusMode::Full,
            CameraFocusArg::Upper => CameraFocusMode::Upper,
            CameraFocusArg::Face => CameraFocusMode::Face,
            CameraFocusArg::Hands => CameraFocusMode::Hands,
        }
    }
}

impl From<WasdModeArg> for CameraControlMode {
    fn from(value: WasdModeArg) -> Self {
        match value {
            WasdModeArg::Orbit => CameraControlMode::Orbit,
            WasdModeArg::Freefly => CameraControlMode::FreeFly,
        }
    }
}

impl From<CameraModeArg> for CameraMode {
    fn from(value: CameraModeArg) -> Self {
        match value {
            CameraModeArg::Off => CameraMode::Off,
            CameraModeArg::Vmd => CameraMode::Vmd,
            CameraModeArg::Blend => CameraMode::Blend,
        }
    }
}

impl From<CameraAlignPresetArg> for CameraAlignPreset {
    fn from(value: CameraAlignPresetArg) -> Self {
        match value {
            CameraAlignPresetArg::Std => CameraAlignPreset::Std,
            CameraAlignPresetArg::AltA => CameraAlignPreset::AltA,
            CameraAlignPresetArg::AltB => CameraAlignPreset::AltB,
        }
    }
}

impl From<TextureSamplingArg> for TextureSamplingMode {
    fn from(value: TextureSamplingArg) -> Self {
        match value {
            TextureSamplingArg::Nearest => TextureSamplingMode::Nearest,
            TextureSamplingArg::Bilinear => TextureSamplingMode::Bilinear,
        }
    }
}

impl From<TextureVOriginArg> for TextureVOrigin {
    fn from(value: TextureVOriginArg) -> Self {
        match value {
            TextureVOriginArg::Gltf => TextureVOrigin::Gltf,
            TextureVOriginArg::Legacy => TextureVOrigin::Legacy,
        }
    }
}

impl From<TextureSamplerModeArg> for TextureSamplerMode {
    fn from(value: TextureSamplerModeArg) -> Self {
        match value {
            TextureSamplerModeArg::Gltf => TextureSamplerMode::Gltf,
            TextureSamplerModeArg::Override => TextureSamplerMode::Override,
        }
    }
}

impl From<BackendArg> for RenderBackend {
    fn from(value: BackendArg) -> Self {
        match value {
            BackendArg::Cpu => RenderBackend::Cpu,
            BackendArg::Gpu => RenderBackend::Gpu,
        }
    }
}

impl From<ToggleArg> for bool {
    fn from(value: ToggleArg) -> Self {
        matches!(value, ToggleArg::On)
    }
}

impl From<AudioReactiveArg> for AudioReactiveMode {
    fn from(value: AudioReactiveArg) -> Self {
        match value {
            AudioReactiveArg::Off => AudioReactiveMode::Off,
            AudioReactiveArg::On => AudioReactiveMode::On,
            AudioReactiveArg::High => AudioReactiveMode::High,
        }
    }
}

impl From<CinematicCameraArg> for CinematicCameraMode {
    fn from(value: CinematicCameraArg) -> Self {
        match value {
            CinematicCameraArg::Off => CinematicCameraMode::Off,
            CinematicCameraArg::On => CinematicCameraMode::On,
            CinematicCameraArg::Aggressive => CinematicCameraMode::Aggressive,
        }
    }
}
