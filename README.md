# Terminal Miku 3D (Rust, CPU-Only)

CPU-only terminal renderer for 3D meshes/animations (ASCII/Braille).

## Features

- Software rasterizer (projection + triangle rasterization + z-buffer)
- GLB/glTF + OBJ loading
- Skeletal animation playback (glTF skin)
- `cargo start` Ratatui 단계형 위저드
- `rodio` 기반 BGM 루프 재생 (`mp3`/`wav`)
- 오디오 시계 기반 애니메이션 동기화 + 오프셋 보정
- 자동 셀 비율 추정 + 비율 캘리브레이션
- 해상도 적응형 대비/안개 보정 (폰트 확대 시 가시성 강화)

## Commands

```bash
cargo start
cargo start --dir assets/glb --music-dir assets/music
cargo start --sync-offset-ms 120 --sync-speed-mode auto --cell-aspect-mode auto
cargo run -- run --scene cube --mode ascii --fps-cap 30
cargo run -- run --scene glb --glb /path/to/model.glb --anim 0 --mode ascii --fps-cap 30 --cell-aspect 0.5 --cell-aspect-mode auto
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
3. 렌더 옵션 (`모드`, `FPS`, `대비`, `동기화`, `비율 모드`, `폰트 프리셋`)
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
- `+` / `-`: orbit speed
- `[` / `]`: zoom out / zoom in
- `i`/`k`, `j`/`l`, arrow keys: framing move
- `c`: framing/zoom reset
- `,` / `.` / `/`: sync offset -10ms / +10ms / 0ms
- `v`: contrast preset cycle (`adaptive-low -> adaptive-normal -> adaptive-high -> fixed`)

## Asset Policy

- Repository에는 모델/모션 자산을 포함하지 않습니다.
- 로컬 자산(`assets-local/`) 기준으로 실행하세요.
- PMX/VMD는 오프라인에서 GLB로 변환 후 사용하세요.
- 변환 가이드는 `/Users/user/miku/scripts/convert_mmd_to_glb.md` 참고.
