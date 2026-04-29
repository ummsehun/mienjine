# Terminal Miku 3D - DDD 리팩토링 설계 문서 (phase.md)

> **문서 목적**: 현재 코드베이스의 프로덕션 위험 문제를 DDD(Domain-Driven Design) 방식으로 재아키텍처링하기 위한 설계 명세
>
> **작성일**: 2026-04-30
>
> **현재 상태**: 🔴 Critically Underprotected (P0 위험 2개, P1 위험 1개)
>
> **목표**: 4-6주 리팩토링 후 프로덕션 준비도 달성

---

## 목차

1. [현재 상태 진단 (As-Is)](#1-현재-상태-진단-as-is)
2. [타겟 아키텍처 설계 (To-Be)](#2-타겟-아키텍처-설계-to-be)
3. [Bounded Context 분리](#3-bounded-context-분리)
4. [Aggregate 설계](#4-aggregate-설계)
5. [명시적 에러 처리 체계](#5-명시적-에러-처리-체계)
6. [동기화 정책 재설계](#6-동기화-정책-재설계)
7. [리팩토링 Phase별 계획](#7-리팩토링-phase별-계획)
8. [Migration 전략](#8-migration-전략)

---

## 1. 현재 상태 진단 (As-Is)

### 1.1 문제점 요약

| ID | 위험도 | 영역 | 현재 증상 |
|----|--------|------|-----------|
| **P0-A** | 🔴 CRITICAL | Exception Handling | 109개 unwrap()/expect() + JavaScript `catch (_) {}` |
| **P0-B** | 🔴 CRITICAL | Code Complexity | 2개 1000줄+ 파일, 7개 500줄+ 파일 |
| **P1-A** | 🟠 HIGH | Concurrency | Race condition 3개 (localClockSec, WebSocket, HTTP sync) |
| **P2-A** | 🟡 MEDIUM | Architecture | unsafe 코드 2개소, 순환 의존성 위험 |
| **P2-B** | 🟡 MEDIUM | Policy | 테스트 부재, CI/CD 미비, 로깅 비구조화 |

### 1.2 현재 모듈 구조

```
terminal_miku3d/
├── src/
│   ├── main.rs                 # 6줄 - 단순 진입점 ✅
│   ├── lib.rs                  # 10줄 - 모듈 export ✅
│   │
│   ├── assets/                 # ❌ 경계 모호
│   │   ├── loader/             # GLB/PMX/OBJ 로딩 (646줄 파일 존재)
│   │   ├── vmd_motion/        # VMD 모션 파싱
│   │   └── vmd_camera/        # VMD 카메라
│   │
│   ├── engine/                 # ❌ 도메인 혼합
│   │   ├── animation/         # 스키네틱 + 모프 애니메이션
│   │   ├── skeleton/           # 본/IK
│   │   ├── pmx_rig/           # PMX 물리 + IK (840줄 파일)
│   │   ├── camera_track/      # 카메라 트랙
│   │   ├── pipeline/          # 렌더 파이프라인 (683줄 파일)
│   │   ├── scene/             # 씬 관리 (types.rs 475줄)
│   │   └── math/              # 기하 연산
│   │
│   ├── render/                 # ❌ 책임 혼재
│   │   ├── backend/           # CPU/GPU 추상화
│   │   ├── renderer/          # ASCII/Braille 래스터라이저 (1000줄+)
│   │   ├── cpu/               # 소프트웨어 래스터라이저
│   │   ├── gpu/               # wgpu 백엔드
│   │   ├── common/            # 색상/머티리얼/텍스처
│   │   ├── morph/             # 모프 타겟
│   │   └── frame/            # 프레임 버퍼
│   │
│   └── runtime/               # ❌ 거대化和
│       ├── app/               # 앱 실행 (545줄)
│       ├── cli/               # CLI 파싱
│       ├── config/            # 설정 (641줄 preset.rs)
│       ├── state/             # 런타임 상태
│       ├── interaction/       # 사용자 입력 (1022줄 state.rs)
│       ├── rendering/         # 렌더 루프
│       ├── sync/              # 오디오 동기화
│       ├── terminal/          # 터미널 캡
│       └── platform_io/      # 터미널/그래픽 I/O
```

### 1.3 현재 에러 처리 구조

```rust
// ❌ 문제점 1: 일관성 없는 에러 처리
// Rust에서는 anyhow::Error에만 의존
fn load_glb(path: &str) -> anyhow::Result<Scene> { ... }

// ❌ 문제점 2: unwrap() 남용 (109개)
// GPU 리소스 접근 시 초기화 가정
let pipeline = self.pipeline.as_ref().unwrap();

// ❌ 문제점 3: JavaScript에서 에러 무시
try { ... } catch (_) { }  // ← silent failure

// ❌ 문제점 4: 전역 panic 핸들러 부재
// main.rs: anyhow::Result<()> 사용하지만 panic 복구 로직 없음
```

### 1.4 현재 동기화 구조

```javascript
// ❌ 문제점: Race Condition
let localClockSec = 0;  // 공유 가변 상태

function tick() {
    localClockSec += delta;  // 여러 컨텍스트에서 동시 접근
}

function applySync(data) {
    localClockSec = data.offset;  // HTTP/WebSocket 양쪽에서 수정
}
```

---

## 2. 타겟 아키텍처 설계 (To-Be)

### 2.1 DDD 기반Bounding Context 분리

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                        Terminal Miku 3D - Bounded Contexts                    │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  ┌──────────────┐    ┌──────────────┐    ┌──────────────┐                   │
│  │   Asset      │    │   Engine     │    │   Render     │                   │
│  │   Context    │    │   Context    │    │   Context    │                   │
│  │              │    │              │    │              │                   │
│  │ • GLB Loader │    │ • Animation   │    │ • Rasterizer │                   │
│  │ • PMX Loader │    │ • Skeleton   │    │ • Backend    │                   │
│  │ • VMD Parser │    │ • Scene Mgmt │    │ • Presenter │                   │
│  │              │    │ • Physics    │    │              │                   │
│  └──────┬───────┘    └──────┬───────┘    └──────┬───────┘                   │
│         │                   │                   │                           │
│         └───────────────────┴───────────────────┘                           │
│                             │                                               │
│                      ┌──────▼───────┐                                       │
│                      │   Runtime    │                                       │
│                      │   Context    │                                       │
│                      │              │                                       │
│                      │ • CLI         │                                       │
│                      │ • Config      │                                       │
│                      │ • State Mgmt  │                                       │
│                      │ • Sync       │                                       │
│                      │ • UI (TUI)   │                                       │
│                      └──────────────┘                                       │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 2.2 Aggregate 설계

#### Aggregate 1: `AssetAggregate` (자산 관리)

```
AssetAggregate
├── Entity: Asset (식별자: path)
├── Entity: GLBAsset
├── Entity: PMXAsset
├── Entity: VMDMotion
├── Entity: VMDCamera
├── ValueObject: TextureMetadata
├── ValueObject: AnimationClip
└── Repository: AssetRepository (trait)
```

**책임**: 자산 로딩, 캐싱, 변환

#### Aggregate 2: `EngineAggregate` (코어 엔진)

```
EngineAggregate
├── Entity: Scene
│   ├── Entity: Model (mesh + material)
│   ├── Entity: Skeleton
│   └── Entity: Camera
├── Entity: AnimationController
│   ├── ValueObject: AnimationClip
│   ├── ValueObject: MorphTarget
│   └── ValueObject: BoneTransform
├── Entity: PhysicsWorld
├── ValueObject: Transform
└── Repository: SceneRepository (trait)
```

**책임**: 씬 그래프 관리, 애니메이션 재생, 물리 시뮬레이션

#### Aggregate 3: `RenderAggregate` (렌더링)

```
RenderAggregate
├── Entity: RenderPipeline
│   ├── ValueObject: Projection
│   ├── ValueObject: RasterConfig
│   └── ValueObject: ShadingParams
├── Entity: RenderTarget
├── ValueObject: FrameBuffer
├── ValueObject: ColorPalette
└── Repository: RenderRepository (trait)
```

**책임**: 프레임 렌더링, 출력 프로토콜 관리

#### Aggregate 4: `RuntimeAggregate` (애플리케이션)

```
RuntimeAggregate
├── Entity: AppContext
│   ├── Entity: CLIArguments
│   ├── Entity: ConfigFile
│   └── Entity: RuntimeState
├── Entity: SyncController
│   ├── ValueObject: SyncProfile
│   └── ValueObject: SyncOffset
├── Entity: UIControl
└── Service: ApplicationService
```

**책임**: CLI 파싱, 설정 관리, 동기화, UI 제어

---

## 3. Bounded Context 분리

### 3.1 Context Map

```
┌────────────────────────────────────────────────────────────────────┐
│                         Context Map                                  │
├────────────────────────────────────────────────────────────────────┤
│                                                                      │
│   Asset Context          Engine Context          Render Context     │
│   ─────────────          ──────────────          ──────────────     │
│         │                      │                       │              │
│         │    «uses»           │    «uses»            │              │
│         ▼                      ▼                       ▼              │
│   ┌──────────┐          ┌──────────┐          ┌──────────┐         │
│   │  Scene   │          │  Scene   │          │  Scene   │         │
│   │ (read)   │          │ (write)  │          │ (read)   │         │
│   └──────────┘          └──────────┘          └──────────┘         │
│                               │                                     │
│                               │    «publishes»                       │
│                               ▼                                     │
│                        ┌──────────┐                                 │
│                        │ Domain   │                                 │
│                        │ Events   │                                 │
│                        └──────────┘                                 │
│                               │                                     │
│                               ▼                                     │
│                        Runtime Context                               │
│                        (구독 및 조율)                                │
│                                                                      │
└────────────────────────────────────────────────────────────────────┘
```

### 3.2anticoded Consistency Boundaries

**Rule 1**: `AssetContext`는 `EngineContext`에만 의존, 그 역은 불가
**Rule 2**: `EngineContext`는 오직 Domain Event로 `RuntimeContext`와 통신
**Rule 3**: `RenderContext`는 읽기 전용으로 `Scene`을 사용
**Rule 4**: 모든 Context 간 통신은 Publish-Subscribe 패턴 사용

### 3.3 모듈 구조 (TARGET)

```
src/
├── domain/                           # DDD Core (순수 도메인 로직)
│   ├── asset/
│   │   ├── mod.rs
│   │   ├── entities/                 # AssetAggregate
│   │   │   ├── mod.rs
│   │   │   ├── asset.rs
│   │   │   ├── glb_asset.rs
│   │   │   ├── pmx_asset.rs
│   │   │   └── vmd_asset.rs
│   │   ├── value_objects/
│   │   │   ├── texture_meta.rs
│   │   │   └── animation_clip.rs
│   │   ├── repositories/            # interfaces only
│   │   │   └── asset_repository.rs
│   │   └── errors/
│   │       └── asset_error.rs
│   │
│   ├── engine/
│   │   ├── mod.rs
│   │   ├── entities/                 # EngineAggregate
│   │   │   ├── mod.rs
│   │   │   ├── scene.rs
│   │   │   ├── model.rs
│   │   │   ├── skeleton.rs
│   │   │   ├── animation_controller.rs
│   │   │   └── camera.rs
│   │   ├── value_objects/
│   │   │   ├── transform.rs
│   │   │   ├── bone_transform.rs
│   │   │   └── morph_target.rs
│   │   ├── repositories/
│   │   │   └── scene_repository.rs
│   │   └── errors/
│   │       └── engine_error.rs
│   │
│   ├── render/
│   │   ├── mod.rs
│   │   ├── entities/                 # RenderAggregate
│   │   │   ├── mod.rs
│   │   │   ├── render_pipeline.rs
│   │   │   ├── render_target.rs
│   │   │   └── frame_buffer.rs
│   │   ├── value_objects/
│   │   │   ├── projection.rs
│   │   │   ├── raster_config.rs
│   │   │   └── color_palette.rs
│   │   ├── repositories/
│   │   │   └── render_repository.rs
│   │   └── errors/
│   │       └── render_error.rs
│   │
│   ├── runtime/
│   │   ├── mod.rs
│   │   ├── entities/
│   │   │   ├── app_context.rs
│   │   │   ├── cli_arguments.rs
│   │   │   ├── config_file.rs
│   │   │   └── sync_controller.rs
│   │   ├── value_objects/
│   │   │   ├── sync_profile.rs
│   │   │   └── sync_offset.rs
│   │   ├── services/
│   │   │   └── application_service.rs
│   │   └── errors/
│   │       └── runtime_error.rs
│   │
│   └── shared/                        # Shared Kernel
│       ├── mod.rs
│       ├── domain_event.rs
│       ├── entity.rs
│       ├── value_object.rs
│       └── aggregate_root.rs
│
├── application/                       # Application Services (Orchestration)
│   ├── asset/
│   │   ├── mod.rs
│   │   ├── asset_loader_service.rs
│   │   └── asset_preprocessor_service.rs
│   ├── engine/
│   │   ├── mod.rs
│   │   ├── scene_service.rs
│   │   ├── animation_service.rs
│   │   └── physics_service.rs
│   ├── render/
│   │   ├── mod.rs
│   │   ├── rendering_service.rs
│   │   └── presentation_service.rs
│   └── runtime/
│       ├── mod.rs
│       ├── cli_service.rs
│       ├── config_service.rs
│       └── sync_coordinator_service.rs
│
├── infrastructure/                    # Infrastructure (Implementations)
│   ├── asset/
│   │   ├── mod.rs
│   │   ├── glb_loader_impl.rs
│   │   ├── pmx_loader_impl.rs
│   │   ├── vmd_parser_impl.rs
│   │   └── asset_repository_impl.rs
│   ├── engine/
│   │   ├── mod.rs
│   │   ├── scene_repository_impl.rs
│   │   └── math_utils.rs
│   ├── render/
│   │   ├── mod.rs
│   │   ├── cpu_rasterizer_impl.rs
│   │   ├── gpu_renderer_impl.rs
│   │   └── render_repository_impl.rs
│   ├── runtime/
│   │   ├── mod.rs
│   │   ├── cli_parser_impl.rs
│   │   ├── config_loader_impl.rs
│   │   ├── gascii_config_parser.rs
│   │   └── terminal_output_impl.rs
│   ├── persistence/
│   │   └── sync_profile_repository.rs
│   └── logging/
│       ├── mod.rs
│       └── tracing_logger.rs
│
├── interfaces/                        # Interface Adapters (Primary)
│   ├── cli/
│   │   └── terminal_interface.rs
│   ├── tui/
│   │   └── ratatui_adapter.rs
│   └── preview/
│       └── web_preview_adapter.rs
│
├── lib.rs
└── main.rs
```

---

## 4. Aggregate 설계

### 4.1 공통 기반 Traits

```rust
// src/domain/shared/entity.rs
pub trait Entity<T> {
    fn id(&self) -> &T;
}

impl<T: PartialEq> PartialEq for dyn Entity<T> {
    fn eq(&self, other: &dyn Entity<T>) -> bool {
        self.id() == other.id()
    }
}
```

### 4.2 AssetAggregate 상세 설계

```rust
// src/domain/asset/entities/asset.rs
pub trait Asset: Entity<AssetId> + Send + Sync {
    fn path(&self) -> &Path;
    fn metadata(&self) -> &TextureMetadata;
    fn is_loaded(&self) -> bool;
    fn unload(&mut self);
}

// src/domain/asset/entities/glb_asset.rs
pub struct GLBAsset {
    id: AssetId,
    path: PathBuf,
    metadata: TextureMetadata,
    scene: Option<Scene>,
    animations: Vec<AnimationClip>,
}

impl Asset for GLBAsset {
    fn path(&self) -> &Path { &self.path }
    fn metadata(&self) -> &TextureMetadata { &self.metadata }
    fn is_loaded(&self) -> bool { self.scene.is_some() }
    fn unload(&mut self) { self.scene = None; }
}

// src/domain/asset/repositories/asset_repository.rs
pub trait AssetRepository: Send + Sync {
    fn load(&self, id: &AssetId) -> impl Future<Output = Result<Box<dyn Asset>, AssetError>>;
    fn preload(&self, ids: &[AssetId]) -> impl Future<Output = Result<Vec<Box<dyn Asset>>, AssetError>>;
    fn evict(&self, id: &AssetId) -> Result<(), AssetError>;
}
```

### 4.3 Domain Event 설계

```rust
// src/domain/shared/domain_event.rs
pub trait DomainEvent: Send + Sync {
    fn event_type(&self) -> &'static str;
    fn occurred_at(&self) -> DateTime<Utc>;
}

// 예시 Events
pub struct AssetLoadedEvent {
    id: AssetId,
    asset_type: AssetType,
    occurred_at: DateTime<Utc>,
}

pub struct AnimationStartedEvent {
    clip_id: String,
    started_at: f32,
}

pub struct SyncOffsetAdjustedEvent {
    old_offset_ms: i32,
    new_offset_ms: i32,
}
```

### 4.4 Aggregate Root 규칙

```rust
// src/domain/shared/aggregate_root.rs
pub trait AggregateRoot: Entity<u64> + Send + Sync {
    fn pending_events(&self) -> &[Box<dyn DomainEvent>];
    fn clear_pending_events(&mut self);
}

// 규칙: Aggregate 상태 변경 시 반드시 Domain Event 발생
// 규칙: Event는 불변 (immutable)
// 규칙: 외부에서는 Aggregate를 통째로 수정 불가 → Command를 통해间接적 변경
```

---

## 5. 명시적 에러 처리 체계

### 5.1 계층별 에러 타입 설계

```rust
// src/domain/asset/errors/asset_error.rs
#[derive(Debug, Clone, Error)]
pub enum AssetError {
    #[error("asset not found: {path}")]
    NotFound { path: PathBuf },

    #[error("unsupported format: {format}")]
    UnsupportedFormat { format: String },

    #[error("corrupted asset: {reason}")]
    Corrupted { reason: String },

    #[error("loading failed: {source}")]
    LoadingFailed { source: String },

    #[error("io error: {source}")]
    IoError { source: String },
}

// src/domain/engine/errors/engine_error.rs
#[derive(Debug, Clone, Error)]
pub enum EngineError {
    #[error("scene not found: {id}")]
    SceneNotFound { id: u64 },

    #[error("animation clip not found: {clip_id}")]
    AnimationNotFound { clip_id: String },

    #[error("invalid bone hierarchy: {reason}")]
    InvalidBoneHierarchy { reason: String },

    #[error("physics init failed: {source}")]
    PhysicsInitFailed { source: String },
}

// src/domain/render/errors/render_error.rs
#[derive(Debug, Clone, Error)]
pub enum RenderError {
    #[error("pipeline not initialized")]
    PipelineNotInitialized,

    #[error("gpu device error: {reason}")]
    GpuDeviceError { reason: String },

    #[error("frame buffer overflow")]
    FrameBufferOverflow,

    #[error("unsupported resolution: {width}x{height}")]
    UnsupportedResolution { width: u32, height: u32 },

    #[error("backend error: {backend}")]
    BackendError { backend: String, source: String },
}

// src/domain/runtime/errors/runtime_error.rs
#[derive(Debug, Clone, Error)]
pub enum RuntimeError {
    #[error("invalid cli arguments: {reason}")]
    InvalidCliArgs { reason: String },

    #[error("config parse error: {path}")]
    ConfigParseError { path: PathBuf, source: String },

    #[error("sync failed: {reason}")]
    SyncFailed { reason: String },

    #[error("terminal not supported")]
    TerminalNotSupported,

    #[error("panic occurred: {message}")]
    PanicOccurred { message: String },
}
```

### 5.2 Result 타입 규칙

```rust
// 규칙 1: Domain 계층에서는 thiserror 사용
use thiserror::Error;

#[derive(Debug, Error)]
pub enum MyError { ... }

// 규칙 2: unwrap() / expect() 프로덕션 코드에서 금지
// ✅ Good
fn load_asset(&self, id: &AssetId) -> Result<Arc<dyn Asset>, AssetError> {
    self.repository.load(id)?
}

// ❌ Bad (P0-A 문제)
fn load_asset(&self, id: &AssetId) -> Arc<dyn Asset> {
    self.repository.load(id).unwrap()  // 🚨 FORBIDDEN
}

// 규칙 3: 테스트 코드에서는 .unwrap() 허용 but #[should_panic] 명시
#[test]
#[should_panic(expected = "asset not found")]
fn test_load_nonexistent_asset() {
    let repo = MockAssetRepository::new();
    repo.load(&AssetId::new("nonexistent")).unwrap();
}
```

### 5.3 전역 Panic Handler

```rust
// src/infrastructure/runtime/panic_handler.rs
use std::panic;

pub fn setup_panic_hook() {
    let original_hook = panic::take_hook();
    panic::set_hook(Box::new(move |panic_info| {
        // 1. 로그 기록
        eprintln!("[PANIC] {}", panic_info);

        // 2. 백트레이스 저장
        let backtrace = std::backtrace::Backtrace::capture();
        eprintln!("[BACKTRACE]\n{}", backtrace);

        // 3. 상태 파일 저장 (디버깅용)
        if let Some(state) = capture_runtime_state() {
            let _ = save_panic_state(&state);
        }

        // 4. 원본 hook 호출
        original_hook(panic_info);
    }));
}
```

### 5.4 JavaScript 에러 처리

```javascript
// src/interfaces/preview/js/error-handler.js

// ✅ Good: 명시적 에러 처리
try {
    const state = await fetch('/state');
    if (!state.ok) {
        throw new Error(`HTTP ${state.status}: ${state.statusText}`);
    }
    return await state.json();
} catch (error) {
    console.error('[SyncError] Failed to fetch state:', error);
    // 폴백 로직
    return fallbackState();
}

// ❌ Bad (P0-A 문제)
try {
    const state = await fetch('/state');
    return await state.json();
} catch (_) { }  // 🚨 FORBIDDEN - silent failure
```

---

## 6. 동기화 정책 재설계

### 6.1 Sync Aggregate 설계

```rust
// src/domain/runtime/entities/sync_controller.rs

#[derive(Debug, Clone)]
pub struct SyncState {
    offset_ms: i32,
    drift_ema: f32,
    hard_snap_count: u32,
    last_sync_at: Option<DateTime<Utc>>,
}

impl SyncState {
    pub fn new() -> Self {
        Self {
            offset_ms: 0,
            drift_ema: 0.0,
            hard_snap_count: 0,
            last_sync_at: None,
        }
    }

    // 원자적 업데이트 (Race Condition 방지)
    pub fn adjust_offset(&mut self, new_offset_ms: i32) -> SyncOffsetAdjustedEvent {
        let old = self.offset_ms;
        self.offset_ms = new_offset_ms;
        self.last_sync_at = Some(Utc::now());
        SyncOffsetAdjustedEvent {
            old_offset_ms: old,
            new_offset_ms: new_offset_ms,
        }
    }

    pub fn record_drift(&mut self, drift_ms: f32, alpha: f32) {
        self.drift_ema = self.drift_ema * (1.0 - alpha) + drift_ms * alpha;
    }
}

// ✅ 이제 Mutex로 보호된 공유 상태로 사용
use std::sync::Mutex;

pub struct SyncController {
    state: Mutex<SyncState>,  // ← 명시적 잠금
}

impl SyncController {
    pub fn new() -> Self {
        Self {
            state: Mutex::new(SyncState::new()),
        }
    }

    pub fn adjust_offset(&self, new_offset_ms: i32) -> Result<SyncOffsetAdjustedEvent, ()> {
        let mut state = self.state.lock().map_err(|_| ())?;
        Ok(state.adjust_offset(new_offset_ms))
    }
}
```

### 6.2 JavaScript 동기화 안전화

```javascript
// src/interfaces/preview/js/sync-controller.js

class SyncController {
    #state = {
        offset_ms: 0,
        lastSyncAt: null,
    };
    #pendingSync = null;  // 폴링 큐

    // ✅ 원자적 업데이트
    adjustOffset(newOffsetMs) {
        this.#state.offset_ms = newOffsetMs;
        this.#state.lastSyncAt = Date.now();
    }

    // ✅ 폴링 중 중복 방지
    async fetchState() {
        if (this.#pendingSync) {
            return this.#pendingSync;  // 이미 진행 중인 요청 재사용
        }

        this.#pendingSync = this._doFetch()
            .finally(() => { this.#pendingSync = null; });

        return this.#pendingSync;
    }

    async _doFetch() {
        const res = await fetch('/state');
        if (!res.ok) throw new Error(`HTTP ${res.status}`);
        const data = await res.json();
        this.adjustOffset(data.sync_offset_ms);
        return data;
    }
}

// ✅ WebSocket도同一个 controller 사용
class WebSocketSync {
    #controller;
    #ws = null;

    connect(url) {
        this.#ws = new WebSocket(url);
        this.#ws.onmessage = (event) => {
            const data = JSON.parse(event.data);
            this.#controller.adjustOffset(data.offset);
        };
    }
}
```

---

## 7. 리팩토링 Phase별 계획

### Phase 1: 기반 구축 (1-2주차)

**목표**: Domain 계층 분리, 에러 타입 정의

| Task | 파일/모듈 | 검증 기준 |
|------|-----------|-----------|
| Shared Kernel 작성 | `domain/shared/` | Entity, ValueObject, DomainEvent traits |
| AssetAggregate 작성 | `domain/asset/` | thiserror 기반 에러 타입 |
| EngineAggregate 작성 | `domain/engine/` | thiserror 기반 에러 타입 |
| RenderAggregate 작성 | `domain/render/` | thiserror 기반 에러 타입 |
| RuntimeAggregate 작성 | `domain/runtime/` | thiserror 기반 에러 타입 |
| Panic Hook 인프라 | `infrastructure/runtime/` | 테스트에서 panic 포착 확인 |

### Phase 2: 에러 처리 체계 전환 (2-3주차)

**목표**: unwrap() 일괄 제거, 명시적 에러 처리

| Task | 파일/모듈 | 검증 기준 |
|------|-----------|-----------|
| 프로덕션 unwrap() 제거 | 50개 파일 | `grep -r "\.unwrap()" src/` 0건 |
| expect() → ? 전환 | pipeline, renderer | thiserror 타입 사용 |
| JavaScript try/catch 정리 | preview-web/ | console.error 로깅 확인 |
| 전역 panic handler | main.rs | panic 발생 시 로그 파일 생성 |

### Phase 3: 복잡도 해소 (3-4주차)

**목표**: 대형 파일 분할, Aggregate 정리

| Task | 파일 | 분할 후 타겟 |
|------|------|-------------|
| panels.rs 분할 | 1,079줄 | header/content/footer/shared (각 200줄 이하) |
| state.rs 분할 | 1,022줄 | wizard_state/types/validators |
| pmx_rig/mod.rs 분할 | 840줄 | ik_chain/physics/joints |
| pipeline/mod.rs 분할 | 683줄 | stages/strategies/validators |

### Phase 4: 동기화 안전화 (4-5주차)

**목표**: Race condition 제거, 원자적 업데이트

| Task | 검증 기준 |
|------|-----------|
| SyncController Mutex 추가 | 동시 접근 테스트 통과 |
| JavaScript SyncController 클래스化 | setInterval 중복 방지 |
| ContinuousSyncState 잠금 추가 | Arc<Mutex<...>> 패턴 적용 |

### Phase 5: 통합 및 문서화 (5-6주차)

**목표**: 전체 통합, CI/CD Setup, 문서화

| Task | 검증 기준 |
|------|-----------|
| 전체 Aggregate 통합 테스트 | cargo test 100% 통과 |
| clippy lint 적용 | warnings 0건 |
| cargo fmt 적용 | formatting 일관성 |
| 문서화 완료 | API 문서 생성 |

---

## 8. Migration 전략

### 8.1 Strangler Fig 패턴

기존 코드를 한 번에 교체하지 않고, 새로운 구조를 옆에 구축하고 점진적으로 이전합니다.

```
Phase 1:平行運行
┌─────────────────────┐     ┌─────────────────────┐
│    Old Codebase      │     │    New DDD Layer     │
│                     │     │                     │
│  src/assets/loader/ │ ←→  │  domain/asset/      │
│                     │     │  application/asset/ │
│                     │     │  infrastructure/     │
└─────────────────────┘     └─────────────────────┘

Phase 2: routing 전환
┌─────────────────────┐     ┌─────────────────────┐
│    Old Codebase      │     │    New DDD Layer     │
│                     │     │                     │
│  (대부분 deprecated) │     │  Main entry points  │
│                     │     │  src/lib.rs         │
└─────────────────────┘     └─────────────────────┘

Phase 3: 정리
┌─────────────────────┐
│    New DDD Layer     │
│                     │
│  Clean architecture │
└─────────────────────┘
```

### 8.2 검증 기준

각 Phase完成后, 以下 기준을 만족해야 다음 Phase 진행:

1. **컴파일 성공**: `cargo build --release` 0에러
2. **테스트 통과**: `cargo test` 100% 통과
3. **lint 통과**: `cargo clippy` warnings < 5
4. **기능 동일**: 기존 기능 regression 없음

### 8.3 Rollback 계획

각 Phase 완료 시 Git tag 점들어:

```bash
git tag -a phase1-complete -m "Phase 1: Domain layer established"
git tag -a phase2-complete -m "Phase 2: Error handling refactored"
# ...
```

문제 발생 시 마지막 tag로 rollback:

```bash
git checkout phaseX-complete && cargo build
```

---

## 부록: 현재 파일 → DDD 모듈 Mapping

| 현재 파일 | Target 모듈 | 비고 |
|----------|------------|------|
| `assets/loader/gltf_load.rs` | `infrastructure/asset/glb_loader_impl.rs` | |
| `assets/loader/pmx_load.rs` | `infrastructure/asset/pmx_loader_impl.rs` | |
| `engine/pmx_rig/mod.rs` | `domain/engine/entities/skeleton.rs` + `domain/engine/entities/physics.rs` | |
| `engine/pipeline/mod.rs` | `domain/engine/entities/pipeline.rs` | |
| `render/renderer/mod.rs` | `domain/render/entities/render_pipeline.rs` | |
| `runtime/interaction/start_ui/panels.rs` | `interfaces/tui/panels/` | 분할 대상 |
| `runtime/interaction/start_ui/state.rs` | `domain/runtime/entities/wizard_state.rs` | 분할 대상 |
| `runtime/config/preset.rs` | `domain/runtime/entities/config_preset.rs` | |
| `preview-web/app.js` | `interfaces/preview/js/sync-controller.js` | |

---

## 참고 문서

- [DDD Reference](https://www.domainlanguage.com/ddd/reference/)
- [Rust Error Handling](https://doc.rust-lang.org/book/ch09-00-error-handling.html)
- [thiserror crate](https://github.com/dtolnay/thiserror)
- [tracing crate](https://github.com/tokio-rs/tracing)
