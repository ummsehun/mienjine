# Terminal Miku 3D (Rust, CPU-Only)

CPU-first terminal renderer for 3D meshes/animations (ASCII/Braille) with optional experimental GPU backend.

## Features

- Software rasterizer (projection + triangle rasterization + z-buffer)
- GLB/glTF + OBJ loading
- GLB material color pipeline (`baseColorFactor` + `COLOR_0` + `TEXCOORD_0` + texture sampling)
- Skeletal animation playback (glTF skin)
- Morph target(표정) 애니메이션 재생 (`weights` 채널)
- `cargo start` Ratatui 단계형 위저드
- `rodio` 기반 BGM 루프 재생 (`mp3`/`wav`)
- 오디오 시계 기반 애니메이션 동기화 + 오프셋 보정
- 자동 셀 비율 추정 + 비율 캘리브레이션
- 해상도 적응형 대비/안개 보정 (폰트 확대 시 가시성 강화)
- 출력 경로 최적화(Diff presenter + synchronized update)
- 저강도 그라데이션 배경(LOD에 따라 full/reduced/minimal)
- Braille 2x4 서브픽셀 합성 + safe 가시성 보정
- ANSI truecolor 하이브리드(테마 기반) + mono fallback
- 오디오 에너지 반응(밝기/배경/카메라 미세 펄스)
- 자동 시네마틱 카메라(전신/상반신/근접 전환)
- 동적 품질 제어(`balanced/cinematic/smooth`) + LOD 자동 조정
- 터미널 과대 해상도 보호(내부 렌더 상한 600x180)
- `gpu`(macOS + feature build) 실험 경로 + 미지원 환경 CPU fallback

## Commands

```bash
cargo start
cargo start-dev
cargo start --dir assets/glb --music-dir assets/music
cargo start --sync-offset-ms 120 --sync-speed-mode auto --cell-aspect-mode auto
cargo start --mode braille --color-mode ansi --braille-profile safe --theme theater --audio-reactive on --cinematic-camera on --reactive-gain 0.35 --perf-profile balanced --detail-profile ultra --center-lock-mode root --camera-focus auto --material-color on --texture-sampling nearest --backend cpu
cargo run -- run --scene cube --mode ascii --fps-cap 30
cargo run -- run --scene glb --glb /path/to/model.glb --fps-cap 0
cargo run -- run --scene glb --glb /path/to/model.glb --anim 0 --mode ascii --fps-cap 30 --cell-aspect 0.5 --cell-aspect-mode auto
cargo run --features gpu -- run --scene glb --glb /path/to/model.glb --mode braille --color-mode ansi --braille-profile normal --theme neon --perf-profile smooth --backend gpu
cargo run -- run --scene obj --obj /path/to/model.obj --mode ascii --fps-cap 30
cargo run -- inspect --glb /path/to/model.glb
cargo run -- bench --scene cube --seconds 10
cargo run -- bench --scene obj --obj /path/to/model.obj --seconds 10
cargo run -- bench --scene glb-static --glb /path/to/model.glb --seconds 10
cargo run -- bench --scene glb-anim --glb /path/to/model.glb --anim 0 --seconds 10
```

## Start Wizard (Ratatui)

`cargo start`는 `assets/glb` + `assets/music`를 스캔하고 5단계 위저드를 엽니다.

1. 모델 선택
2. 음악 선택 (`없음` 포함)
3. 렌더 옵션 (`모드`, `성능/디테일 프로필`, `백엔드`, `중앙 고정/기준`, `카메라 포커스`, `재질색상/텍스처 샘플링`, `스테이지 레벨`, `FPS`, `대비`, `동기화`, `비율 모드`, `폰트 프리셋`)
4. 비율 캘리브레이션 (원형 프리뷰 + trim 조절)
5. 확인/실행 (감지/적용 비율, clip/audio 길이, speed factor 표시)

공통 키:

- `Enter`: 현재 단계 확정/다음
- `Esc`: 이전 단계 (1단계에서는 취소)
- `q`: 즉시 취소
- `Tab` / `Shift+Tab`: 보조 포커스/이동

## 해상도 대응

- `Wide`: `>= 140x40`
- `Normal`: `>= 100x28`
- `Compact`: 그 미만
- `60x18` 미만이면 실행 차단 화면을 표시하고 리사이즈 시 자동 복귀

## Gascii.config

`/Users/user/miku/Gascii.config`를 자동 로드합니다.

지원 키:

- `ui_language = ko|en`
- `font_preset_enabled = true|false`
- `font_preset_steps = N`
- `cell_aspect_mode = auto|manual`
- `cell_aspect_trim = 0.70..1.30`
- `contrast_profile = adaptive|fixed`
- `color_mode = mono|ansi`
- `braille_profile = safe|normal|dense`
- `theme = theater|neon|holo`
- `audio_reactive = off|on|high`
- `cinematic_camera = off|on|aggressive`
- `reactive_gain = 0.0..1.0`
- `perf_profile = balanced|cinematic|smooth`
- `detail_profile = perf|balanced|ultra`
- `backend = cpu|gpu` (`gpu-preview`도 레거시 alias로 인식)
- `exposure_bias = -0.5..0.8`
- `center_lock = true|false`
- `center_lock_mode = root|mixed`
- `camera_focus = auto|full|upper|face|hands`
- `material_color = true|false`
- `texture_sampling = nearest|bilinear`
- `braille_aspect_compensation = 0.70..1.30`
- `stage_level = 0..4`
- `stage_reactive = true|false`
- `sync_offset_ms = -5000..5000`
- `sync_speed_mode = auto|realtime`
- `triangle_stride = 1..16`
- `min_triangle_area_px2 = 0.0..16.0`

레거시 키도 호환됩니다:

- `ghostty_font_reset` -> `font_preset_enabled`
- `ghostty_font_steps` -> `font_preset_steps`

주의:

- Ghostty 폰트 프리셋은 `start` 위저드의 렌더 옵션에서 토글했을 때만 적용됩니다.
- 런타임(`run`) 중 폰트 단축키는 사용하지 않습니다.

예시:

```ini
ui_language = ko
font_preset_enabled = true
font_preset_steps = 2
cell_aspect_mode = auto
cell_aspect_trim = 1.00
contrast_profile = adaptive
sync_offset_ms = 0
sync_speed_mode = auto
color_mode = ansi
braille_profile = safe
theme = theater
audio_reactive = on
cinematic_camera = on
reactive_gain = 0.35
perf_profile = balanced
detail_profile = balanced
backend = cpu
exposure_bias = 0.00
center_lock = true
center_lock_mode = root
camera_focus = auto
material_color = true
texture_sampling = nearest
braille_aspect_compensation = 0.90
stage_level = 2
stage_reactive = true
triangle_stride = 2
min_triangle_area_px2 = 0.08
```

## Music Playback

- `start`에서 선택한 음악은 `rodio`로 루프 재생됩니다.
- 기본 동기화는 `sync_speed_mode=auto`일 때 `clip_duration / audio_duration` 계수로 애니메이션 시간을 자동 보정합니다.
- 오디오 초기화 실패 시 렌더링은 계속 진행되며 무음으로 동작합니다.

## Runtime Controls

- `q` or `Esc`: quit
- `o`: orbit camera toggle
- `r`: model auto-spin toggle (non-animated scene)
- `+` / `-`: stage level up/down (`0..4`)
- `e` / `E`: exposure bias down/up
- `f`: center lock on/off toggle
- `x` / `z`: orbit speed up/down
- `[` / `]`: zoom out / zoom in
- `i`/`k`, `j`/`l`, arrow keys: framing move
- `c`: framing/zoom reset
- `,` / `.` / `/`: sync offset -10ms / +10ms / 0ms
- `v`: contrast preset cycle (`adaptive-low -> adaptive-normal -> adaptive-high -> fixed`)
- `b`: braille profile cycle (`safe -> normal -> dense`)
- `n`: color mode toggle (`mono <-> ansi`)
- `p`: cinematic camera toggle (`off <-> on`)
- `g` / `G`: reactive gain -/+ (`0.0..1.0`)

추가:

- `--fps-cap 0`은 프레임 제한 해제(unlimited)입니다.

추가 동작:

- `-` 연타로 저가시성이 지속되면 watchdog이 자동 복구(FullBody + framing reset + exposure 보정)
- `center_lock=on`일 때 pan 키(`i/j/k/l/u/m`, 화살표)는 비활성화
- 출력 I/O 실패가 누적되면 `mono + full fallback` 모드로 자동 전환

## Asset Policy

- Repository에는 모델/모션 자산을 포함하지 않습니다.
- 로컬 자산(`assets-local/`) 기준으로 실행하세요.
- PMX/VMD는 오프라인에서 GLB로 변환 후 사용하세요.
- 변환 가이드는 `/Users/user/miku/scripts/convert_mmd_to_glb.md` 참고.
