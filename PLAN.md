# Tusk — AI-native Postgres Client

> **목표**: Postgres 클라이언트 카테고리에서 dominant player 자리 노리는 OSS 데스크탑 앱. AI를 처음부터 1급 시민으로 박은 Postico/TablePlus의 후계자.
>
> **북극성 지표**: GitHub 10k+ stars (1년 내). 수익화 X. 별 받기 자체가 1차 목표.

---

## 1. 왜 지금, 왜 이 카테고리

기존 Postgres GUI 클라이언트들의 상태:

- **Postico** — 예쁘지만 macOS 전용, AI 없음, 업데이트 느림.
- **TablePlus** — 다 있지만 클라우드 sync 강요로 욕먹음. AI 없음.
- **pgAdmin** — 무겁고 못생김. enterprise 의무로만 씀.
- **DBeaver** — Java + 복잡 + UI 2010년대. 모든 DB 지원해서 깊이 없음.
- **Beekeeper Studio** — Electron, multi-DB, OSS. 가까운 경쟁자.

빈자리:

1. **Postgres-first**, 다른 DB 안 함 → 깊이로 이김
2. **AI 처음부터** 박혀있음 (Cursor 패턴) → 자연어 → SQL, EXPLAIN 해석, 인덱스 추천
3. **Tauri 2 + Rust** → 30MB 바이너리, 100ms 부팅 → "또 다른 Electron 앱" 거부감 회피
4. **local-first**, BYOK → 기업 신뢰 + hobbyist 수용
5. **pgvector 1급 시민** → AI/RAG 만드는 사람들 즉시 끌림

타이밍이 좋음: AI 데브툴 폭증 + Cursor 검증된 BYOK 패턴 + Tauri 2 안정화 + Postgres가 어느 때보다 dominant.

---

## 2. 이름

**Tusk** — 코끼리 엄니. Postgres 마스코트(코끼리)와 시각적으로 즉시 연결. 4글자, 1음절, 글로벌 발음.

다른 후보들이 다 먹혀있어서 Tusk로 확정. 도메인/GitHub org 확보 우선:

- `tusk.dev`, `tusk.app`, `usetusk.com`, `tusk-db.com` 중 살아있는 거 즉시 매수
- GitHub: `tusk`, `tusk-app`, `usetusk`, `gettusk` 순으로 시도
- npm: 안 쓸 가능성 크지만 `tusk-cli` 정도는 예약

마케팅 카피:

- _"Postgres, with intelligence."_
- _"Sharp like a tusk. Built in Rust."_
- _"The Postgres client that thinks ahead."_

---

## 3. 기술 스택

```
┌─────────────────────────────────────────────┐
│              Tauri 2.0 (껍데기)              │
│  - 윈도우/메뉴/트레이/번들/배포                │
│  - Rust ↔ WebView 사이 IPC                  │
│  ┌────────────────────────────────────────┐ │
│  │  Frontend (React + TypeScript)         │ │
│  │  - Tailwind CSS + shadcn/ui            │ │
│  │  - Monaco Editor (SQL 에디터)           │ │
│  │  - TanStack Table (가상화 result grid) │ │
│  │  - Zustand (state)                     │ │
│  │  - Vercel AI SDK (LLM 스트리밍)         │ │
│  │  - ECharts/visx (EXPLAIN 시각화)        │ │
│  └────────────────────────────────────────┘ │
│                    ↕ invoke()                │
│  ┌────────────────────────────────────────┐ │
│  │  Backend (Rust)                        │ │
│  │  - sqlx 또는 tokio-postgres            │ │
│  │  - rusqlite (로컬 앱 상태)              │ │
│  │  - keyring (OS keychain 연결정보)       │ │
│  │  - serde (직렬화)                       │ │
│  │  - reqwest (LLM API 호출, 옵션)         │ │
│  └────────────────────────────────────────┘ │
└─────────────────────────────────────────────┘
```

LLM 호출은 **프론트에서 직접** 하는 게 기본 (Vercel AI SDK의 streaming UX 활용). Rust 측 LLM 호출은 백그라운드 작업(스키마 임베딩 등) 시에만.

### 왜 Tauri 2

- 30MB 바이너리 vs Electron 150MB+
- 콜드 스타트 100~500ms vs Electron 1~3s
- idle 메모리 30~80MB vs Electron 150~300MB
- "Built with Rust" 첫인상 — 이 카테고리에서 별 1k vs 5k 가르는 변수
- 보안 모델 박스로 옴

### 왜 React (Vue/Svelte 아님)

- shadcn/ui + Radix 풍부한 에코시스템
- Monaco/CodeMirror/TanStack 등 power tool 컴포넌트가 React-first
- 사용자(우리 본인)가 매일 쓰는 스택

### Rust 학습 부담

본인이 짜야 할 Rust:

- Postgres connection pool 관리 (sqlx)
- `#[tauri::command]` 함수 ~20개 (쿼리 실행, 스키마 fetch, 파일 IO 등)
- 직렬화 (serde 자동)
- 에러 핸들링 (anyhow / thiserror)

깊이 안 들어감. 1~2주 학습으로 충분.

---

## 4. 핵심 기능 (v1)

### 기본 GUI 클라이언트 기능 (테이블 스테이크)

- 연결 관리 (URL/host 입력, OS keychain 저장, SSH tunnel 옵션)
- 스키마 트리 (DB → schema → table → column)
- SQL 에디터 (Monaco, syntax highlight, 자동완성, 멀티 탭)
- Result grid (가상화, 수백만 row 부드럽게, CSV/JSON export)
- 트랜잭션 컨트롤 (BEGIN/COMMIT/ROLLBACK 명시적)
- 쿼리 히스토리 (시간순)

### Tusk만의 차별 기능 (AI 1급 시민)

#### a. **자연어 → SQL** (Cursor의 Cmd+K 패턴)

- 에디터 안에서 `Cmd+K` → 자연어 입력 → SQL 생성 → 검토 후 실행
- 스키마 자동 컨텍스트 (관련 테이블만 RAG로 추출)
- diff view로 기존 SQL 수정 제안

#### b. **EXPLAIN 시각화 + AI 해석**

- `EXPLAIN (ANALYZE, BUFFERS, FORMAT JSON)` 결과를 트리/타임라인으로 시각화
- pgMustard 스타일 + LLM이 한 줄 요약 ("순차 스캔 50% 시간 소비, idx_users_email 누락 추정")
- 인덱스 추천 (낮은 카디널리티는 거름)

#### c. **쿼리 히스토리 의미 검색**

- 실행한 쿼리 모두 자동 임베딩 (로컬, OpenAI 또는 sentence-transformers)
- "지난주에 user랑 order 조인했던 그 쿼리" 자연어 검색
- 즐겨찾기 쿼리 + 폴더 정리

#### d. **pgvector 1급 시민**

- vector 컬럼 자동 감지
- 임베딩 시각화 (UMAP/t-SNE 2D 산점도)
- "이 row와 비슷한 거 찾기" 우클릭 메뉴 (코사인 유사도 자동 쿼리)
- HNSW 인덱스 상태 표시

#### e. **Slow query 자동 분석**

- `pg_stat_statements` 한 화면에 표시
- 가장 느린 쿼리에 AI 진단 자동 실행
- 인덱스 추천 + 리라이트 제안

#### f. **로컬 LLM 옵션 (Ollama)**

- BYOK 기본 (OpenAI/Anthropic/Gemini)
- 프라이버시 옵션으로 Ollama 로컬 모델 (Llama 3.1 8B 등)
- 기업 사용자 끌어들이는 결정적 차별점

### v1에 _안 넣는_ 것

- Multi-DB 지원 (MySQL/SQLite 등) — 절대 안 함. Postgres-first 깊이로 이김.
- 클라우드 sync — 로컬 우선. 나중에 옵션으로.
- 협업 기능 — 1인 도구로 시작.
- 모바일 — 데스크탑만.

---

## 5. AI 통합 설계

### BYOK (Bring Your Own Key)

- 사용자 본인 API key 입력 (OpenAI/Anthropic/Gemini/Ollama)
- 키는 OS keychain에 저장 (rust `keyring` crate)
- 호스팅 LLM 안 함 → 비용 폭탄 회피, 신뢰 확보
- Cursor/Aider/Cline 패턴 그대로

### 컨텍스트 전달

스키마/히스토리를 LLM 컨텍스트에 잘 넣는 게 _기술적 핵심_:

1. **연결 시 한 번**: `information_schema` 전체 fetch → 테이블별 `CREATE TABLE` 문 + 코멘트 + 컬럼 통계 (`pg_stats`) 추출 → 임베딩 → 로컬 SQLite + sqlite-vec 또는 일반 SQLite + 인메모리 검색.

2. **쿼리 생성 시**: 사용자 입력 임베딩 → 관련 테이블 top-K 검색 → DDL을 system prompt에 첨부.

3. **히스토리 활용**: 과거 실행한 비슷한 쿼리도 컨텍스트에 (few-shot처럼).

### 프롬프트 정책

- 시스템 프롬프트에 Postgres 버전, 사용 중인 확장(`pgvector`, `pg_trgm` 등) 명시
- "destructive 쿼리(DROP/TRUNCATE/DELETE without WHERE)는 항상 사용자에게 명시적 확인 요구"
- 답변은 SQL 코드 블록 + 짧은 설명만 (장황한 LLM 톤 금지)

### Tool calling

LLM이 직접 호출 가능한 도구:

- `get_table_schema(name)` — DDL 반환
- `sample_rows(table, limit)` — 5~10 row 샘플
- `get_index_list(table)` — 기존 인덱스
- `explain_query(sql)` — EXPLAIN ANALYZE 실행 (사용자 확인 후)

### 비용/지연 관리

- 쿼리 생성: GPT-4o-mini / Claude Haiku 기본 (빠르고 싸고 충분)
- 복잡한 EXPLAIN 해석: GPT-4o / Claude Sonnet (옵션)
- 임베딩: text-embedding-3-small (싸고 충분)
- 사용자가 모델 자유 선택 가능

---

## 6. 차별화 매트릭스

| 기능                    | Postico | TablePlus | DBeaver  | Beekeeper     | **Tusk**      |
| ----------------------- | ------- | --------- | -------- | ------------- | ------------- |
| 멀티 OS                 | macOS만 | O         | O        | O             | **O**         |
| AI 자연어→SQL           | X       | X         | 플러그인 | X             | **★ 1급**     |
| EXPLAIN 시각화          | X       | 약함      | X        | X             | **★ 시각+AI** |
| pgvector 지원           | X       | X         | X        | X             | **★ 1급**     |
| 바이너리 크기           | 작음    | 중        | 큼       | 큼 (Electron) | **30MB**      |
| 콜드 스타트             | 빠름    | 빠름      | 느림     | 중            | **빠름**      |
| OSS                     | X       | X         | O        | O             | **O**         |
| 로컬 LLM                | X       | X         | X        | X             | **★**         |
| 쿼리 히스토리 의미 검색 | X       | 텍스트만  | X        | X             | **★**         |

★ = Tusk만의 차별점.

---

## 7. 10k star 전략

### 첫인상 폭격 (출시 day 0)

- README 첫 화면이 30초 GIF (자연어 → SQL → 결과 흐름)
- 랜딩 페이지 (tusk.dev): 5초 영상 자동 재생, 다운로드 버튼 즉시 보임
- 다운로드 후 90초 안에 첫 쿼리 실행되어야 함 (셋업 마찰 0)
- 코끼리 엄니 모노그램 로고, 다크 테마 강한 첫인상

### 출시 채널

- **Show HN**: "Tusk – AI-native Postgres client built with Rust + Tauri"
- **r/postgres, r/rust, r/programming**
- **Twitter**: 영문 + 한국어 양쪽. 데모 영상 첨부.
- **블로그 글** (zerry.co.kr): 만든 이야기 + 함정 + 결정. 블로그 자체가 트래픽 채널.
- **Hacker News의 weekly hiring/show 스레드** 매주 댓글
- **Awesome lists PR**: awesome-postgres, awesome-tauri 등에 PR

### 출시 후 운영

- GitHub issue 응답 24시간 내 (별 100→1k 구간 결정적)
- changelog.md를 정성껏 (사용자가 업데이트 기대하게)
- 매월 minor 릴리스 + 블로그 글 1편
- 디스코드 또는 GitHub Discussions 공식 채널

### 별 1k → 10k 가는 길

- influencer adoption: 한국/영어권 데브 인플루언서에게 일찍 보냄 (Theo, Primeagen, 토스 ENG 블로그 등)
- 큰 기능 1개당 별 +500~1000 기대 (예: pgvector 시각화)
- 한국 IT 매체(GeekNews, 프로그래머스 블로그) 노출
- ProductHunt 출시 (2~3개월 후 정도, 첫 기능 안정화 후)

---

## 8. 8주 v1 로드맵

### Week 1 — 인프라

- Tauri 2 프로젝트 init (`npm create tauri-app@latest`)
- 폴더 구조, ESLint/Prettier, Rust clippy
- GitHub repo 생성 (private, 출시 직전 public)
- 도메인/이메일 확보
- 디자인 시스템: Tailwind config, 색 토큰, 다크모드

### Week 2 — 연결 + 기본 쿼리

- 연결 추가/저장 (keychain), 연결 풀 관리
- 스키마 트리 사이드바 (lazy load)
- SQL 에디터 (Monaco) + 멀티 탭
- 기본 result grid (TanStack Table 가상화)

### Week 3 — Result 인라인 편집 (DataGrip 패턴) + 트랜잭션

이 주가 Tusk의 결정적 차별점 중 하나. 디테일이 많으니 풀파워로 한 주 잡음.

#### 인라인 편집 UX (핵심)

- 셀 더블클릭 → 인라인 에디터, 타입에 따라 다른 위젯
- 편집된 셀은 노란 highlight + 원래 값 호버로 보임
- 여러 row 동시 편집 가능, 행 추가/삭제도 같은 패턴
- 상단/하단 툴바에 pending change 카운트 (`(2 pending)`)
- **Preview** 버튼 → 생성될 SQL 미리보기 모달 (UPDATE/INSERT/DELETE 묶음)
- **Submit** → BEGIN → statements → 결과 OK면 COMMIT 버튼 노출, 실패하면 자동 ROLLBACK + 에러 표시
- **Revert** → 모든 pending 편집 취소, 원본 값 복원
- 키보드: `Tab`/`Shift+Tab`/`Enter`로 셀 이동, `Esc`로 편집 취소, `Cmd+S`로 submit

#### 타입별 인라인 에디터

- `text`/`varchar` → 텍스트 (개행 가능 멀티라인 옵션)
- `int`/`numeric` → 숫자 입력 + 범위 검증 (`numeric(10,2)`이면 자릿수 검증)
- `bool` → 토글 / 체크박스
- `timestamp`/`timestamptz` → 날짜+시간 picker + 타임존 표시
- `date` → 날짜 picker
- `enum` → dropdown (DB에서 enum 값 fetch)
- FK references 컬럼 → dropdown (참조 테이블 lookup, 검색 가능)
- `jsonb`/`json` → 펼쳐지는 JSON 에디터 (Monaco 미니 인스턴스, syntax highlight + validation)
- `bytea` → hex/base64 토글, 큰 값은 파일로 export 옵션
- `uuid` → 텍스트 + "Generate UUID" 버튼
- `vector` (pgvector) → 읽기 전용 + 차원 표시 (편집은 UX 어려우니 일단 X)
- 모든 nullable 컬럼 → 항상 "Set NULL" 명시적 버튼

#### Primary Key 감지 (편집 가능 여부의 게이트)

- 결과셋 row를 UPDATE/DELETE하려면 PK 필요
- 쿼리 결과 컬럼 메타데이터에서 source table + PK 추론 (`pg_attribute` 조회)
- PK 없는 결과셋(JOIN, GROUP BY, 서브쿼리 등) → 편집 disabled, "Read-only result (no primary key)" 표시
- 단일 테이블 SELECT라도 PK 컬럼이 결과에 빠져있으면 편집 disabled
- 편집 가능한 결과는 grid 좌상단에 작은 ✏️ 인디케이터

#### 충돌 감지 (DataGrip보다 한 단계 위)

- 편집 시작 시점의 row 원본 값 저장
- Submit 시점에 `WHERE pk = X AND col1 = original AND col2 = original ...` 형태로 UPDATE 생성 (선택적 옵션)
- Affected rows = 0이면 "Row was modified by someone else" 경고 → 사용자가 force/cancel 결정
- 또는 더 단순한 방식: submit 직전에 다시 SELECT해서 비교 후 diff 표시

#### 명시적 트랜잭션 모드

- 상단 토글: **Auto-commit ON / OFF**
- OFF 상태에서 모든 쿼리(직접 SQL + 인라인 편집 + Cmd+K AI 쿼리) → 트랜잭션 안에서 실행
- 트랜잭션 활성 시 항상 보이는 인디케이터: `🟡 Transaction in progress (3 stmts)`
- 명시적 COMMIT / ROLLBACK 버튼 (단축키: `Cmd+Shift+C` / `Cmd+Shift+R`)
- 커밋 안 된 채로 앱 닫으려 하면 경고 모달
- 트랜잭션 내 실행된 모든 statement 히스토리 사이드 패널에 보임

#### 그 외 Week 3 항목

- CSV / JSON / SQL INSERT export (선택 row만 또는 전체)
- 키보드 단축키 셋업 (Cmd+Enter 실행, Cmd+T 새 탭, Cmd+W 탭 닫기, Cmd+P 명령 팔레트 등)
- 쿼리 히스토리 기본 저장 (SQLite, 시간순)
- 셀 우클릭 컨텍스트 메뉴: Copy, Copy as INSERT, Set NULL, Filter by this value, "Find similar" (vector 컬럼)

#### 위험/주의

- 인라인 편집은 사용자 데이터 직접 변경 → 한 번 망가지면 신뢰 즉사
- destructive 쿼리(DELETE/TRUNCATE/DROP) confirmation은 Week 4 AI 모달과 통합 설계
- Submit 시 항상 명시적 트랜잭션으로 감싸기 (auto-commit OFF여도, ON이여도)
- 큰 결과셋(10만 row+) 편집 시 메모리 폭발 방지 — 편집된 row만 추적
- 컬럼 타입이 unknown(custom type)인 경우 텍스트 fallback + 경고

### Week 4 — AI 1차 (자연어 → SQL)

- BYOK 설정 화면
- Cmd+K 입력 → 자연어 → SQL diff
- 스키마 컨텍스트 RAG (임베딩 인덱스 빌드)
- destructive 쿼리 confirmation 모달

### Week 5 — EXPLAIN 시각화

- EXPLAIN ANALYZE 트리 시각화
- 노드 클릭 → 상세 패널
- AI 해석 사이드바
- 인덱스 추천 (낮은 카디널리티 필터)

### Week 6 — pgvector 통합

- vector 컬럼 자동 감지
- UMAP 2D 시각화 (WebAssembly umap-js)
- "유사한 row 찾기" 우클릭
- HNSW/IVFFlat 인덱스 상태

### Week 7 — 히스토리 의미 검색 + Polishing

- 쿼리 히스토리 임베딩 (자동, 백그라운드)
- 의미 검색 UI (Cmd+P 같은 팔레트)
- 즐겨찾기 + 폴더
- 디자인 폴리싱 (애니메이션, 빈 상태, 로딩 스켈레톤)

### Week 8 — 출시 준비

- 앱 사이닝 (macOS notarization, Windows 인증서)
- 자동 업데이트 (Tauri updater)
- 랜딩 페이지 + 데모 영상
- README 완성, 스크린샷 6장
- Show HN 초안 + 블로그 글
- **Day 0 출시**

---

## 9. 새 세션 시작 명령어

```bash
cd /Users/cyj/workspace/personal/tusk

# 1. Tauri 2 프로젝트 init
npm create tauri-app@latest .
# - Project name: tusk
# - Identifier: dev.tusk.app
# - Frontend: TypeScript / React
# - Package manager: npm
# - UI template: React (vite)

# 2. 디펜던시 추가
npm install -D tailwindcss postcss autoprefixer @types/node
npx tailwindcss init -p
npm install class-variance-authority clsx tailwind-merge lucide-react
npm install @radix-ui/react-dialog @radix-ui/react-dropdown-menu # 기타 shadcn 의존성
npm install @monaco-editor/react @tanstack/react-table zustand
npm install ai @ai-sdk/openai @ai-sdk/anthropic # AI SDK

# 3. shadcn/ui 셋업
npx shadcn@latest init
npx shadcn@latest add button input dialog dropdown-menu

# 4. Rust 디펜던시 (src-tauri/Cargo.toml)
# sqlx = { version = "0.8", features = ["runtime-tokio", "postgres", "uuid", "chrono", "json"] }
# tokio = { version = "1", features = ["full"] }
# serde = { version = "1", features = ["derive"] }
# anyhow = "1"
# keyring = "3"
# rusqlite = { version = "0.32", features = ["bundled"] }

# 5. 첫 실행
npm run tauri dev
```

---

## 10. 결정 미뤄둔 것 (새 세션에서 정할 것)

- [ ] 정확한 도메인 (tusk.dev / tusk.app / usetusk.com 중)
- [ ] GitHub org/repo 이름
- [ ] 라이선스 (MIT vs Apache 2.0 vs AGPL)
  - 권장: **MIT** — 채택률 높이는 데 유리. 수익화 안 할 거면 AGPL은 과해.
- [ ] 로고 (코끼리 엄니 단순화 모노그램)
- [ ] 색 팔레트 (다크모드 우선, 액센트 색)
- [ ] CodeMirror 6 vs Monaco — Monaco가 자동완성/IntelliSense 강하지만 무거움. CodeMirror 6은 가볍고 모던. **현재 권장: Monaco** (DX 우위)
- [ ] 임베딩 — OpenAI text-embedding-3-small vs 로컬 sentence-transformers (Tauri 사이드카) — v1은 OpenAI BYOK가 단순
- [ ] 폰트 (JetBrains Mono / Geist Mono / SF Mono)

---

## 11. 위험 요소 (의식적으로 관리)

1. **Rust 학습 곡선**: Postgres 연결 + IPC 정도면 충분. 깊이 빠지면 안 됨. 막히면 Electron으로 fallback 가능.
2. **macOS notarization**: 첫 출시 가장 큰 함정. Apple Developer 계정($99/yr) 필요. Windows 인증서도 비싸($300+/yr) — 처음엔 unsigned + Smart Screen 우회 가이드로 출시 가능.
3. **유지보수 burnout**: 1인 OSS 메인테이너의 90% 사망 원인. 첫 3개월 PR/issue 응답 빠르게, 그 후엔 의도적으로 응답 페이스 조절.
4. **TablePlus가 AI 추가**: 가장 큰 외부 위협. Tusk가 OSS + Postgres-first + Rust 우위로 차별화 유지.
5. **AI hallucination**: SQL 잘못 생성해서 사용자 데이터 날아가면 신뢰 즉사. destructive 쿼리는 항상 명시적 confirmation, dry-run 옵션, undo 가능한 트랜잭션 강제.

---

## 12. 본인 동기 점검

10k star가 _진짜_ 동력인지, 아니면 *매일 쓰는 도구*가 동력인지 솔직히 판단할 것. 후자가 답이어야 1년 끌고 갈 수 있음. 별이 안 와도 본인이 매일 쓰면 멈추지 않음. 별만 동력이면 3개월 안 dropoff.

이 프로젝트는 본인 케이스에서 **둘 다 만족시킬 가능성이 가장 높은 후보**라서 선택된 것. 매일 SQL 쓰니까 본인용 도구로 가치 즉시, AI/Rust/Postgres 카테고리 hot이니까 별 가능성도 큼.

---

## 부록 A — 영감 받을 도구들

- **Cursor** — BYOK + Cmd+K 패턴
- **Linear** — 디자인 폴리싱 기준
- **Raycast** — 키보드 우선 power tool UX
- **Postico** — Postgres 클라이언트 미니멀리즘
- **TablePlus** — 기능 셋 (단, UX 함정 피해야 함)
- **pgMustard** — EXPLAIN 시각화 레퍼런스
- **DataGrip (JetBrains)** — 자동완성/스키마 인텔리전스 레퍼런스

## 부록 B — 참고 라이브러리

```
프론트:
- @tauri-apps/api 2.x — IPC
- @monaco-editor/react — SQL 에디터
- @tanstack/react-table — result grid
- @tanstack/react-virtual — 가상화
- ai (Vercel AI SDK) — LLM 스트리밍
- recharts 또는 visx — 차트
- umap-js — 임베딩 시각화 (브라우저)

Rust:
- tauri 2.x
- sqlx 0.8 (postgres feature)
- tokio
- serde + serde_json
- anyhow / thiserror
- keyring
- rusqlite (with sqlite-vec extension 옵션)
- reqwest (LLM API 옵션)
```
