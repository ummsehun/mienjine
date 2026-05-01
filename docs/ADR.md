# Architecture Decision Record (ADR)

## ADR-001: DDD Layered Architecture

**Status**: Accepted

**Date**: 2026-04-30

**Context**: Terminal Miku 3D 프로젝트의 코드 복잡도가 증가하면서 유지보수성이 저하되고 있었음. 1000줄 이상의 파일, unwrap() 남용, 명확하지 않은 에러 처리 등의 문제가 있었음.

**Decision**: DDD(Domain-Driven Design) 4계층 구조를 도입하기로 결정.

- **Domain Layer**: 순수 비즈니스 로직 (Entity, ValueObject, Repository)
- **Application Layer**: 유스케이스 오케스트레이션 (Service)
- **Infrastructure Layer**: 외부 시스템 연동 (Legacy Adapter)
- **Interfaces Layer**: UI/CLI/Web (Phase 7에서 추가 예정)

**Consequences**:
- 코드 복잡도 감소 (500줄 이상 파일 0개 달성)
- 명시적 에러 처리 (unwrap()/expect() 0개 달성)
- 테스트 용이성 향상
- 단기적으로는 파일 수 증가로 인한 관리 비용 증가

## ADR-002: Strangler Fig Pattern

**Status**: Accepted

**Date**: 2026-04-30

**Context**: 기존 코드를 한 번에 리팩토링하는 것은 리스크가 큼. 기능이 망가질 가능성이 높음.

**Decision**: Strangler Fig 패턴을 적용하여 기존 코드를 유지하면서 새 계층을 옆에 구축.

**Consequences**:
- 기존 코드 0 수정으로 안전한 마이그레이션
- 점진적 기능 이전 가능
- 단기적으로는 중복 코드 존재

## ADR-003: thiserror + tracing

**Status**: Accepted

**Date**: 2026-04-30

**Context**: anyhow 남용으로 인해 에러 타입이 불명확하고 디버깅이 어려웠음.

**Decision**: thiserror로 명시적 에러 enum을 정의하고, tracing으로 구조화된 로깅을 도입.

**Consequences**:
- 에러 타입 명확화 (GlbParseError, PmxParseError 등)
- 로그 구조화로 모니터링 용이
- 의존성 증가 (thiserror, tracing)
