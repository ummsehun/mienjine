# Refactor & Integration QA Checklist

이 문서는 대규모 모듈화 이후 회귀를 줄이고, 기능/백엔드 연동/도메인 응집도 품질을 반복적으로 확인하기 위한 체크리스트입니다.

## 1) Gate: 컴파일/테스트/빌드

- [ ] `cargo check`
- [ ] `cargo test`
- [ ] `cargo build --all-targets`
- [ ] 신규 경고(특히 unused import/dead code)가 없는지 확인

## 2) 파일 크기/모듈 경계

- [ ] 500 LOC 이상 Rust 파일이 남아있는지 스캔
- [ ] 분리 후 엔트리 모듈(`mod.rs`/facade)은 조립 역할만 수행하는지 확인
- [ ] 테스트 코드와 런타임 코드를 분리(`*_tests.rs` 또는 `#[cfg(test)] mod tests`)했는지 확인

## 3) 백엔드 연동 (CPU/GPU)

- [ ] `RenderBackend::Cpu` 경로 정상 동작
- [ ] `RenderBackend::Gpu` 실패 시 CPU 폴백 경고/동작 정상
- [ ] 백엔드 선택 로직이 중앙화되어 중복 분기되지 않는지 확인
- [ ] GPU feature on/off 모두 컴파일 가능

## 4) Start/Run/Bench 옵션 연동

- [ ] `resolve_visual_options_for_start/run/bench`가 동일 규칙(기본값/클램프)을 유지하는지 확인
- [ ] CLI 인자 > config 기본값 우선순위가 의도대로 유지되는지 확인
- [ ] bench 전용 강제값(runtime cfg 기반)이 필요한 필드에만 적용되는지 확인

## 5) 도메인 응집도 (Cohesion)

- [ ] `runtime/start_ui/*`: 입력 처리, 상태, 렌더 패널이 역할별로 분리되어 있는지
- [ ] `runtime/config/*`: general/visual/camera/sync 파싱 책임이 교차되지 않는지
- [ ] `render/*`: backend 선택과 raster/material 로직이 계층적으로 분리되어 있는지
- [ ] `engine/*`: animation 샘플링/수학/행렬 계산 책임이 명확히 분리되어 있는지

## 6) 결합도/중복 점검

- [ ] 동일한 매핑/클램프 로직이 여러 파일에 복붙되어 있지 않은지
- [ ] 테스트 유틸이 동일 모듈 안에서 중복 정의되지 않는지
- [ ] 모듈 간 import가 최소 공개 인터페이스만 참조하는지 확인

## 7) 회귀 포인트 스모크 테스트

- [ ] `cargo start -- --help` 또는 start wizard 진입 경로 확인
- [ ] run 경로(대표 scene) 1회 확인
- [ ] bench 경로(대표 scene) 1회 확인
- [ ] stage/camera/sync 옵션 포함 경로에서 panic/오동작 없는지 확인

## 8) 변경 후 기록

- [ ] 이번 분리에서 줄어든 LOC와 남은 대형 파일 목록 기록
- [ ] 발견된 기술부채(중복/복잡도/후속 분리 후보) 기록
- [ ] 다음 phase의 우선순위 파일과 예상 분리 단위 기록
