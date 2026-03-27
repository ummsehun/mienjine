# Terminal Miku 3D (Rust, CPU-Only)

CPU-first terminal renderer for 3D meshes/animations (ASCII/Braille) with optional experimental GPU backend.

## Features

- Software rasterizer (projection + triangle rasterization + z-buffer)
- GLB/glTF + OBJ loading
- GLB material color pipeline (`baseColorFactor` + `COLOR_0` + `TEXCOORD_0` + texture sampling)
- `KHR_texture_transform` 반영(`baseColorTexture` UV transform/texCoord override)
- `preprocess` 서브커맨드(GLB 텍스처 업스케일/샤프닝, 원본 보존)
- 곡/카메라 조합별 sync profile 자동 적용/저장(`assets/sync/profiles.json`)
- Skeletal animation playback (glTF skin)
- Morph target(표정) 애니메이션 재생 (`weights` 채널)
- `cargo start` Ratatui 단계형 위저드 (Model → Music → Stage → Camera → Render → Aspect → Confirm)
- `rodio` 기반 BGM 루프 재생 (`mp3`/`wav`)
- 오디오 시계 기반 애니메이션 동기화 + 오프셋 보정
- 자동 셀 비율 추정 + 비율 캘리브레이션
- 해상도 적응형 대비/안개 보정 (폰트 확대 시 가시성 강화)
- 출력 경로 최적화(Diff presenter + synchronized update)
- Kitty HQ 출력(`kitty-hq`, shm/direct) + text/hybrid 자동 폴백
- 저강도 그라데이션 배경(LOD에 따라 full/reduced/minimal)
- Braille 2x4 서브픽셀 합성 + safe 가시성 보정
- ANSI truecolor 하이브리드(테마 기반) + mono fallback
- 오디오 에너지 반응(밝기/배경/카메라 미세 펄스)
- 자동 시네마틱 카메라(전신/상반신/근접 전환)
- 동적 품질 제어(`balanced/cinematic/smooth`) + LOD 자동 조정
- 터미널 과대 해상도 보호(내부 렌더 상한 4096x2048)
- `assets/stage/{dir}` 스캔 + 상태 분류(`사용 가능`/`PMX 변환 필요`/`사용 불가`)
- `stage.meta.toml` 기반 스테이지 transform(offset/scale/rotation) 적용
- `gpu`(macOS + feature build) 실험 경로 + 미지원 환경 CPU fallback
- `preprocess --preset face-priority` 얼굴 디테일 우선 텍스처 처리(sRGB 샤픈, linear 샤픈 금지)
- `preview /state` sync 메타 + `/mmd-probe` GLB vs PMX/VMD 브릿지 점검 경로

## Commands

```bash
cargo start
cargo start-dev
cargo start --dir assets/glb --music-dir assets/music
cargo start --stage-dir assets/stage --stage auto
cargo start --stage "MyStageDir" --clarity-profile sharp --ansi-quantization off --model-lift 0.14
cargo start --mode ascii --color-mode mono --ascii-force-color on --ansi-quantization off
cargo start --sync-offset-ms 120 --sync-speed-mode auto --cell-aspect-mode auto
cargo start --sync-profile-dir assets/sync --sync-profile-mode auto
cargo start --mode braille --color-mode ansi --braille-profile safe --theme theater --audio-reactive on --cinematic-camera on --reactive-gain 0.35 --perf-profile balanced --detail-profile ultra --center-lock-mode root --camera-focus auto --material-color on --texture-sampling nearest --backend cpu
cargo start --camera-vmd assets/camera/world_is_mine.vmd --camera-mode vmd --camera-align-preset std --camera-unit-scale 0.08 --camera-vmd-fps 30
cargo run -- run --scene cube --mode ascii --fps-cap 30
cargo run -- run --scene glb --glb /path/to/model.glb --fps-cap 0
cargo run -- run --scene glb --glb /path/to/model.glb --anim 0 --mode ascii --fps-cap 30 --cell-aspect 0.5 --cell-aspect-mode auto --stage-dir assets/stage --stage auto
cargo run -- run --scene pmx --pmx /path/to/model.pmx --mode braille --fps-cap 30
cargo run -- run --scene glb --glb /path/to/model.glb --output-mode hybrid --graphics-protocol auto --sync-policy continuous --sync-hard-snap-ms 120 --sync-kp 0.15
cargo run -- run --scene glb --glb /path/to/model.glb --sync-profile-mode write --sync-profile-key world_is_mine
cargo run -- run --scene glb --glb /path/to/model.glb --wasd-mode freefly --freefly-speed 1.2 --camera-look-speed 1.2
cargo run --features gpu -- run --scene glb --glb /path/to/model.glb --mode braille --color-mode ansi --braille-profile normal --theme neon --perf-profile smooth --backend gpu
cargo run -- preprocess --glb assets/glb/miku.glb --out assets/glb/miku_up2.glb --upscale-factor 2 --upscale-sharpen 0.20
cargo run -- preprocess --preset web-parity --glb assets/glb/miku.glb --out assets/glb/miku_web.glb
cargo run -- preprocess --preset face-priority --glb assets/glb/miku.glb --out assets/glb/miku_face.glb --upscale-factor 2 --upscale-sharpen 0.35
cargo run -- run --scene obj --obj /path/to/model.obj --mode ascii --fps-cap 30
cargo run -- inspect --glb /path/to/model.glb
cargo run -- preview --glb /path/to/model.glb --anim 0 --camera-vmd assets/camera/world_is_mine.vmd --camera-mode blend --camera-align-preset alt-a --camera-unit-scale 0.08 --port 8787
cargo run -- bench --scene cube --seconds 10
cargo run -- bench --scene obj --obj /path/to/model.obj --seconds 10
cargo run -- bench --scene glb-static --glb /path/to/model.glb --seconds 10
cargo run -- bench --scene glb-anim --glb /path/to/model.glb --anim 0 --seconds 10
```

## Start Wizard (Ratatui)

`cargo start`는 `assets/glb` + `assets/music` + `assets/stage` + `assets/camera`를 스캔하고 7단계 위저드를 엽니다.

1. 모델 선택
2. 음악 선택 (`없음` 포함)
3. 스테이지 선택 (`스테이지를 선택해 주세요`, 상태 배지 표시)
4. 카메라 선택 (`없음`/`*.vmd` + `mode` + `align preset` + `unit scale`)
5. 렌더 옵션 (`모드`, `성능/디테일/선명도 프로필`, `ANSI 양자화`, `백엔드`, `중앙 고정/기준`, `WASD 모드/속도`, `카메라 포커스`, `재질색상/텍스처 샘플링`, `model_lift`, `edge_accent`, `스테이지 레벨`, `FPS`, `대비`, `동기화`, `비율 모드`, `폰트 프리셋`)
6. 비율 캘리브레이션 (원형 프리뷰 + trim 조절)
7. 확인/실행 (모델/음악/스테이지/카메라 상태, 감지/적용 비율, clip/audio 길이, speed factor 표시)

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
- `ascii_force_color = true|false` (ASCII 모드에서 ANSI 컬러 강제)
- `braille_profile = safe|normal|dense`
- `theme = theater|neon|holo`
- `audio_reactive = off|on|high`
- `cinematic_camera = off|on|aggressive`
- `reactive_gain = 0.0..1.0`
- `perf_profile = balanced|cinematic|smooth`
- `detail_profile = perf|balanced|ultra`
- `backend = cpu|gpu` (`gpu-preview`도 레거시 alias로 인식)
- `clarity_profile = balanced|sharp|extreme`
- `ansi_quantization = q216|off`
- `stage_dir = assets/stage`
- `stage_selection = auto|none|<stage-name>|<path>`
- `exposure_bias = -0.5..0.8`
- `center_lock = true|false`
- `center_lock_mode = root|mixed`
- `wasd_mode = orbit|freefly`
- `freefly_speed = 0.1..8.0`
- `camera_look_speed = 0.1..8.0`
- `camera_dir = assets/camera`
- `camera_selection = none|auto|<name>|<path>`
- `camera_mode = off|vmd|blend`
- `camera_align_preset = std|alt-a|alt-b`
- `camera_unit_scale = 0.01..2.0`
- `camera_vmd_fps = 1..240`
- `camera_vmd_path = assets/camera/<file>.vmd`
- `camera_focus = auto|full|upper|face|hands`
- `material_color = true|false`
- `texture_sampling = nearest|bilinear`
- `texture_v_origin = gltf|legacy`
- `texture_sampler = gltf|override`
- `braille_aspect_compensation = 0.70..1.30`
- `model_lift = 0.02..0.45`
- `edge_accent_strength = 0.0..1.5`
- `bg_suppression = 0.0..1.0`
- `stage_level = 0..4`
- `stage_reactive = true|false`
- `sync_offset_ms = -5000..5000`
- `sync_speed_mode = auto|realtime`
- `sync_policy = continuous|fixed|manual`
- `sync_hard_snap_ms = 10..2000`
- `sync_kp = 0.01..1.0`
- `sync_profile_dir = assets/sync` (프로필 저장 디렉터리)
- `sync_profile_mode = auto|off|write`
- `output_mode = text|hybrid|kitty-hq` (기본 `text`)
- `graphics_protocol = auto|kitty|iterm2|none`
- `kitty_transport = shm|direct`
- `kitty_compression = none|zlib` (`shm`이면 `none` 강제)
- `kitty_internal_res = 640x360|854x480|1280x720`
- `kitty_scale = 0.5..2.0`
- `hq_target_fps = 12..120`
- `subject_exposure_only = true|false`
- `stage_role = sub|off`
- `stage_luma_cap = 0.0..1.0`
- `recover_color = auto|off`
- `upscale_factor = 1|2|4` (`preprocess` 기본값)
- `upscale_sharpen = 0.0..2.0` (`preprocess` 기본값)
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
sync_policy = continuous
sync_hard_snap_ms = 120
sync_kp = 0.15
sync_profile_dir = assets/sync
sync_profile_mode = auto
output_mode = text
recover_color = auto
graphics_protocol = auto
kitty_transport = shm
kitty_compression = none
kitty_internal_res = 640x360
kitty_scale = 1.00
hq_target_fps = 24
subject_exposure_only = true
stage_role = sub
stage_luma_cap = 0.35
upscale_factor = 2
upscale_sharpen = 0.20
color_mode = ansi
ascii_force_color = true
braille_profile = safe
theme = theater
audio_reactive = on
cinematic_camera = on
reactive_gain = 0.35
perf_profile = balanced
detail_profile = balanced
clarity_profile = sharp
ansi_quantization = q216
backend = cpu
stage_dir = assets/stage
stage_selection = auto
exposure_bias = 0.00
center_lock = true
center_lock_mode = root
wasd_mode = freefly
freefly_speed = 1.00
camera_look_speed = 1.00
camera_dir = assets/camera
camera_selection = none
camera_mode = off
camera_align_preset = std
camera_unit_scale = 0.08
camera_vmd_fps = 30.0
camera_vmd_path = assets/camera/world_is_mine.vmd
camera_focus = auto
material_color = true
texture_sampling = nearest
texture_v_origin = gltf
texture_sampler = gltf
braille_aspect_compensation = 1.00
model_lift = 0.12
edge_accent_strength = 0.32
bg_suppression = 0.35
stage_level = 2
stage_reactive = true
triangle_stride = 2
min_triangle_area_px2 = 0.08
```

## Music Playback

- `start`에서 선택한 음악은 `rodio`로 루프 재생됩니다.
- 기본 동기화는 `sync_speed_mode=auto`일 때 `clip_duration / audio_duration` 계수로 애니메이션 시간을 자동 보정합니다.
- 적용 우선순위는 `CLI > sync profile > Gascii.config > default` 입니다.
- `sync_profile_mode=auto|write`에서 런타임 중 `,` `.` `/`로 오프셋을 조정하면 종료 시 `assets/sync/profiles.json`에 write-back 됩니다.
- 오디오 초기화 실패 시 렌더링은 계속 진행되며 무음으로 동작합니다.
- 시작 위저드에서 스테이지를 선택할 수 있으며, `PMX 변환 필요` 항목은 실행 시 차단 + 변환 안내를 출력합니다.
- 스테이지 디렉터리의 PMX 항목은 여전히 GLB로 변환해야 합니다. Blender + MMD Tools로 GLB로 변환한 뒤 같은 `{dir}`에 두면 `사용 가능`으로 자동 전환됩니다.
- 직접 PMX 파일을 열려면 `cargo run -- run --scene pmx --pmx /path/to/model.pmx`를 사용하세요.

## Runtime Controls

- `Esc` or `Q`: quit
- `o`: orbit camera toggle
- `r`: model auto-spin toggle (non-animated scene)
- `W/A/S/D`: freefly 전후/좌우 이동 (`--wasd-mode freefly`)
- `q/e`: freefly 하강/상승 (`--wasd-mode freefly`, freefly에서만 quit 대신 이동)
- `←/→/↑/↓`: freefly 시점 회전 (`camera_look_speed` 반영)
- `+` / `-`: stage level up/down (`0..4`)
- `e` / `E`: exposure bias down/up (`e`는 `--wasd-mode orbit`에서 사용)
- `f`: freefly 토글 (on: VMD/시네마틱 트랙 일시중지, off: 트랙 복귀)
- `t`: center lock on/off toggle
- `x` / `z`: orbit speed up/down
- `[` / `]`: zoom out / zoom in
- `i`/`k`, `j`/`l`, arrow keys: framing move
- `c`: framing/zoom reset
- `,` / `.` / `/`: sync offset -10ms / +10ms / 0ms
- `v`: contrast preset cycle (`adaptive-low -> adaptive-normal -> adaptive-high -> fixed`)
- `b`: braille profile cycle (`safe -> normal -> dense`)
- `n`: color mode toggle (`mono <-> ansi`, 단 ASCII 강제 컬러일 때는 ANSI 고정)
- `p`: cinematic camera toggle (`off <-> on`)
- `g` / `G`: reactive gain -/+ (`0.0..1.0`)

추가:

- `--fps-cap 0`은 프레임 제한 해제(unlimited)입니다.

추가 동작:

- `-` 연타로 저가시성이 지속되면 watchdog이 자동 복구(FullBody + framing reset + exposure 보정)
- `center_lock=on`일 때 pan 키(`i/j/k/l/u/m`, 화살표)는 비활성화
- 출력 I/O 실패가 누적되면 `ansi-truecolor -> ansi-q216 -> mono/full` 순서로 자동 폴백
- `recover_color=auto`이면 성공 프레임 누적 시 `mono -> q216 -> truecolor` 자동 복구

## Web Preview

- `preview` 명령은 로컬 Three.js 비교 경로를 띄웁니다.
- `/state`에는 `sync_offset_ms`, `sync_profile_key`, `sync_profile_hit`, `sync_drift_ema`, `sync_hard_snap_count`가 포함됩니다.
- `/mmd-probe`는 GLTFLoader 경로와 PMX/VMD 브릿지 점검 정보를 노출합니다.
- `output_mode=kitty-hq|hybrid`는 지원 터미널이면 그래픽 프로토콜을 사용하고, 실패 시 즉시 텍스트 경로로 전환합니다.
- 기본 주소: `http://127.0.0.1:8787`
- 동기화 채널:
  - WebSocket `/sync` 20Hz 마스터 클럭 브로드캐스트
  - 연결 불안정 시 HTTP `/sync` 폴링 자동 폴백
- 클라이언트 보정 규칙:
  - 오차 `> 120ms`: 즉시 스냅
  - 그 외: `err * 0.15` 완만 보정

## Asset Policy

- Repository에는 모델/모션 자산을 포함하지 않습니다.
- 로컬 자산(`assets-local/`) 기준으로 실행하세요.
- PMX/VMD는 오프라인에서 GLB로 변환 후 사용하세요.
- 변환 가이드는 `/Users/user/miku/scripts/convert_mmd_to_glb.md` 참고.
- 웹 브릿지/변환 기준 문서는 `/Users/user/miku/docs/web_import_bridge.md` 참고.
