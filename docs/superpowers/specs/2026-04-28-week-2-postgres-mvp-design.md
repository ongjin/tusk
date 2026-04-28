# Week 2 — Postgres MVP + SSH 통합 (design spec)

> **Date:** 2026-04-28
> **Scope:** PLAN.md Week 2 (연결 / 스키마 / 에디터 / result grid) + SSH tunnel 1~3단계
> **Status:** Approved (user, 2026-04-28). Ready for implementation plan.

---

## 1. Goal & success criteria

End user, 처음 켰을 때:

1. `~/.ssh/config`에 등록된 호스트가 모달에 자동 노출.
2. `oci-db` 같은 alias 클릭 → 호스트/포트/유저/키/ProxyJump가 자동 채워짐.
3. **`ssh oci-db`로 평소 접속 가능한 사용자라면 Tusk도 무조건 연결됨.**
4. 연결 후 90초 안에 SQL 실행 → 결과 그리드에 표시.

이 4개가 만족되어야 Week 2 완료. PLAN.md "다운로드 후 90초 안에 첫 쿼리"의 핵심 경로.

## 2. Out of scope (Week 2엔 안 함)

- Cell 편집 / INSERT / UPDATE — Week 3
- 명시적 트랜잭션 (BEGIN/COMMIT/ROLLBACK) — Week 3 (Week 2엔 auto-commit only)
- 쿼리 취소 (`pg_cancel_backend`) — Week 3
- CSV/JSON export — Week 3
- 자연어 → SQL — Week 4
- 스키마-aware 자동완성 — Week 3+
- 스트리밍 결과 (수백만 row 한 번에) — v1.5
- 프론트 단위 테스트 (vitest) — Week 3 시작

## 3. Architecture

```
┌── Frontend (React + zustand) ──────────────────────────┐
│  features/connections    features/schema               │
│  features/editor         features/results              │
│  store/connections.ts    store/tabs.ts                 │
│  store/schema.ts         store/results.ts              │
└────────────────────────────────────────────────────────┘
                  ↕ Tauri invoke()
┌── Rust (src-tauri) ────────────────────────────────────┐
│  commands/{connections, query, schema, ssh}            │
│  db/pool.rs   — Mutex<HashMap<ConnId, ActiveConn>>     │
│  db/state.rs  — rusqlite (메타데이터 영속)              │
│  ssh/config.rs — ~/.ssh/config 파싱 + `ssh -G` 호출    │
│  ssh/tunnel.rs — `ssh -N -L` spawn + readiness check  │
│  secrets.rs    — keyring (비번만)                      │
│  errors.rs     — TuskError (thiserror)                 │
└────────────────────────────────────────────────────────┘
```

**ActiveConnection**:
```rust
struct ActiveConnection {
    pool: PgPool,
    tunnel: Option<TunnelHandle>,  // SSH일 때만 Some
}

struct TunnelHandle {
    child: Child,            // ssh 프로세스
    local_port: u16,         // 포워딩된 로컬 포트
}

impl Drop for TunnelHandle {
    fn drop(&mut self) {
        let _ = self.child.kill();
    }
}
```

앱 종료 시 `tauri::Builder::on_window_event`로 모든 ActiveConnection drop 보장.

## 4. Libraries

| 영역 | 라이브러리 | 사유 |
|---|---|---|
| Postgres | `sqlx 0.8` (runtime-tokio-rustls, postgres, uuid, chrono, json) | PLAN 명시 + compile-time check 가치 |
| 로컬 메타데이터 | `rusqlite 0.32` (bundled) | 단일 파일 SQLite, 마이그레이션 단순 |
| 비밀 보관 | `keyring 3` | OS keychain 통합 (macOS/Win/Linux) |
| 비동기 런타임 | `tokio 1` (full) | sqlx 의존 |
| 에러 | `anyhow` (애플리케이션) + `thiserror` (라이브러리 경계) | Rust 표준 패턴 |
| SSH | **별도 crate 없음** — 시스템 `ssh` 명령 spawn | `ssh -G`가 ProxyJump/Match/Include 다 resolve. 재발명 금지. |

`russh`/`ssh2`를 안 쓰는 이유: ProxyJump 직접 구현 부담, `ssh-agent`/1Password 통합 잡일, 환경별 디버깅 지옥. 사용자 환경에서 `ssh <alias>`가 동작하면 우리 코드도 무조건 동작하게 만드는 게 우선.

## 5. Connection model

### 메타데이터 (SQLite)

```sql
CREATE TABLE connections (
    id           TEXT PRIMARY KEY,    -- UUID
    name         TEXT NOT NULL,
    host         TEXT NOT NULL,
    port         INTEGER NOT NULL DEFAULT 5432,
    db_user      TEXT NOT NULL,
    database     TEXT NOT NULL,
    ssl_mode     TEXT NOT NULL DEFAULT 'require',  -- require/prefer/disable
    ssh_kind     TEXT NOT NULL DEFAULT 'none',     -- none/alias/manual
    ssh_alias    TEXT,                              -- ssh_kind='alias'
    ssh_host     TEXT,                              -- ssh_kind='manual'
    ssh_port     INTEGER,
    ssh_user     TEXT,
    ssh_key_path TEXT,
    created_at   INTEGER NOT NULL,
    updated_at   INTEGER NOT NULL
);
```

비밀번호: keyring `service="tusk"`, `account="conn:{id}"`.

### 활성 풀

```rust
pub struct ConnectionRegistry {
    inner: Arc<Mutex<HashMap<ConnectionId, ActiveConnection>>>,
}
```

연결 끊기 → `inner.lock().remove(&id)` → `ActiveConnection` drop → tunnel 죽음 + pool drop.

## 6. SSH integration

### `~/.ssh/config` 호스트 목록

`ssh/config.rs::list_known_hosts()`:

1. `~/.ssh/config` 파일 read.
2. `Host <pattern>` 라인 파싱. wildcard(`*`, `?`)는 무시 (alias 후보 아님).
3. 각 alias에 대해 `ssh -G <alias>` 호출 → `hostname`, `port`, `user`, `identityfile`, `proxyjump` 추출.
4. UI에 표시할 메타: `{ alias, hostname, user, proxy_jump: Option<String> }`.

`ssh -G`는 `Include`/`Match` 디렉티브까지 다 resolve해주므로 직접 파서 짜지 않음. `~/.ssh/config`는 alias 이름만 추출하는 용도로 가볍게 파싱.

### Tunnel spawn

`ssh/tunnel.rs::open_tunnel(spec) -> Result<TunnelHandle>`:

1. **로컬 free port 할당**: `TcpListener::bind("127.0.0.1:0")?.local_addr()?.port()` → drop.
   - Race condition은 OK — 99% 케이스 안전. 실패 시 retry 1회.
2. **ssh spawn**:
   ```rust
   Command::new("ssh")
       .args([
           "-N",                                             // no remote command
           "-L", &format!("127.0.0.1:{local}:{remote_host}:{remote_port}"),
           "-o", "ServerAliveInterval=30",
           "-o", "ServerAliveCountMax=3",
           "-o", "ExitOnForwardFailure=yes",
           "-o", "BatchMode=no",                             // 키 password prompt 허용
           &target,                                          // alias 또는 user@host
       ])
       .stderr(Stdio::piped())
       .spawn()?
   ```
   - `manual` 모드: `-i <key_path>`, `-p <port>`, `user@host` 추가.
   - `alias` 모드: alias 그대로 마지막 인자 (`ssh -G`로 이미 검증됨).
3. **포트 readiness**: `tokio::net::TcpStream::connect(("127.0.0.1", local))`을 50ms 간격, 최대 5초까지 polling.
   - 성공: `TunnelHandle` 반환.
   - 실패: child kill + stderr 마지막 N줄 포함한 `TuskError::Tunnel` 반환.

### Tunnel 죽었을 때

- Tokio task가 `child.wait()` polling.
- exit 감지 시 → registry에서 해당 connection 무효화 → `tauri::Emitter`로 `connection:lost` 이벤트 발행 → 프론트 사이드바에 빨간 점 + 토스트.
- 자동 재연결은 v1.5 (사용자가 클릭으로 reconnect).

## 7. Connection add UX

```
┌─ New Connection ────────────────────────────┐
│ Tabs: [Direct TCP] [SSH alias] [SSH manual] │
│                                             │
│ ── Direct TCP ──                            │
│   Name        ___________                   │
│   Host        ___________                   │
│   Port        5432                          │
│   User        ___________                   │
│   Password    ___________                   │
│   Database    ___________                   │
│   SSL mode    [require ▼]                   │
│                                             │
│ ── SSH alias ──                             │
│   📋 From ~/.ssh/config:                    │
│   ┌─────────────────────────────────────┐   │
│   │ oci-db        (via app-cf)          │   │
│   │ oci-util      (via app-cf)          │   │
│   │ app-cf                              │   │
│   └─────────────────────────────────────┘   │
│   (selected) oci-db                         │
│                                             │
│   Postgres host  127.0.0.1                  │
│   Postgres port  5432                       │
│   User           ___________                │
│   Password       ___________                │
│   Database       ___________                │
│                                             │
│ ── SSH manual ──                            │
│   SSH host       ___________                │
│   SSH port       22                         │
│   SSH user       ___________                │
│   Key path       [Browse]                   │
│   Postgres host  127.0.0.1                  │
│   ... (이하 동일)                            │
│                                             │
│ [Test connection]    [Cancel] [Save]        │
└─────────────────────────────────────────────┘
```

- Test 버튼: 실제 tunnel 띄움 + `SELECT 1` → 성공/실패 토스트. 모달 안 닫음.
- Save: 메타데이터 SQLite + 비번 keyring. 연결은 자동 시도.

## 8. Schema tree (lazy load)

사이드바:

```
[+ New connection]
─────────────
▼ oci-db (oci-prod)
  ▼ postgres
    ▼ public                     ← 클릭 시 information_schema.tables
      ▶ users
      ▶ orders
      ▶ products
    ▶ analytics
  ▶ template1
▶ local
```

- `commands::schema::list_databases(connId)` → `SELECT datname FROM pg_database WHERE datistemplate = false`.
- `commands::schema::list_schemas(connId, db)` → `information_schema.schemata`.
- `commands::schema::list_tables(connId, db, schema)` → `information_schema.tables`.
- `commands::schema::list_columns(connId, db, schema, table)` → `information_schema.columns` + `pg_catalog.pg_attribute` (PK/NOT NULL).
- 결과는 zustand `store/schema.ts`에 캐시. 새로고침 버튼 우클릭 메뉴.

## 9. SQL editor

- `@monaco-editor/react`, `language="sql"`.
- 멀티 탭 — `store/tabs.ts`:
  ```ts
  type Tab = {
    id: string;
    title: string;          // "Untitled 1" or 첫 줄 추출
    connectionId: string | null;
    sql: string;
    dirty: boolean;
    lastResult?: QueryResult;
  };
  ```
- 단축키:
  - `Cmd+Enter` — 실행
  - `Cmd+T` — 새 탭
  - `Cmd+W` — 탭 닫기 (dirty면 confirm)
  - `Cmd+1..9` — 탭 전환
- 자동완성: Monaco 기본 SQL 키워드만. 스키마-aware는 Week 3+.

## 10. Result grid

- `@tanstack/react-table` + `@tanstack/react-virtual`.
- Read-only.
- 헤더: `{rows} rows · {duration}ms · {connection name}`.
- 컬럼:
  - 폭 드래그 조정 (저장은 v1.5).
  - 클릭 → 정렬 (asc/desc/none, 클라이언트 메모리).
- 셀:
  - `null` → 회색 italic "NULL".
  - JSON/JSONB → 한 줄 truncate, 클릭 시 모달로 expand.
  - `timestamp` / `timestamptz` → ISO 8601.
  - `bytea` → `\x...` hex (truncate).
  - 그 외 → `to_string()`.

### 자동 LIMIT

- 설정 토글 "Append `LIMIT 1000` to bare SELECTs" (default ON).
- 활성화 시 SQL이 `SELECT`이고 `LIMIT` 미포함이면 `LIMIT 1000` append.
- 명시적 LIMIT/OFFSET 있으면 skip.
- 다른 statement (INSERT/UPDATE/DELETE/DDL/CTE)는 무관.

## 11. Data flow — query 한 번

```
사용자 Cmd+Enter (Tab T, connection C, SQL S)
 → frontend: invoke("execute_query", { connectionId: C, sql: S })
 → Rust: registry.lock().get(&C) → pool 획득
 → sqlx::query(&S).fetch_all(&pool).await
 → row → serde_json::Value 직렬화 (PgRow → Vec<(String, JsonValue)>)
 → 결과 응답:
     QueryResult {
       columns: Vec<{ name, type_name }>,
       rows: Vec<Vec<JsonValue>>,
       duration_ms: u64,
       row_count: u64,
     }
 → frontend results store → 그리드 렌더
```

에러 시 `Result<QueryResult, TuskError>`의 Err → 프론트 토스트 + 에디터 하단 빨간 패널.

## 12. Errors

```rust
#[derive(thiserror::Error, Debug, Serialize)]
#[serde(tag = "kind", content = "message")]
pub enum TuskError {
    #[error("Connection failed: {0}")]
    Connection(String),
    #[error("Query failed: {0}")]
    Query(String),
    #[error("SSH tunnel failed: {0}")]
    Tunnel(String),
    #[error("SSH config error: {0}")]
    Ssh(String),
    #[error("State error: {0}")]
    State(String),
    #[error("Secrets error: {0}")]
    Secrets(String),
    #[error("Internal error: {0}")]
    Internal(String),
}
```

- 모든 `#[tauri::command]`는 `Result<T, TuskError>`.
- 프론트 — invoke 래퍼가 `TuskError`를 `Error` 객체로 throw → React Error Boundary + sonner 토스트.

## 13. Testing strategy

**Rust unit (in-process)**:
- `db::state` — rusqlite 인메모리 DB로 CRUD round-trip.
- `ssh::config::extract_aliases` — `~/.ssh/config` fixture 파싱.
- `ssh::config::resolve_via_ssh_g` — `which ssh` 통과 시만 (CI에서는 ubuntu 기본 ssh 사용).
- `errors` 직렬화 round-trip.

**Rust integration (docker)**:
- `tests/postgres.rs` — `docker compose up -d postgres:16-alpine`로 실DB 띄우고 connect/query/schema 명령.
- 첫 슬라이스에 docker compose 파일 추가 (`infra/postgres/docker-compose.yml`).

**Frontend smoke**: 미루고 Week 3에 vitest. 단, **수동 검증 체크리스트**가 슬라이스 끝마다 plan에 포함.

## 14. Folder structure (신규/변경)

```
src/
  features/
    connections/
      ConnectionForm.tsx
      ConnectionList.tsx
      SshHostPicker.tsx
      hooks.ts
      types.ts
    schema/
      SchemaTree.tsx
      SchemaNode.tsx
      hooks.ts
    editor/
      EditorPane.tsx
      EditorTabs.tsx
      keymap.ts
    results/
      ResultsGrid.tsx
      ResultsHeader.tsx
      cells.tsx
  store/
    connections.ts
    tabs.ts
    schema.ts
    results.ts
  lib/
    tauri.ts          # invoke 래퍼 (typed)
    types.ts          # Connection, QueryResult, SshHost, TuskError, ...
    sql.ts            # auto-LIMIT 등 SQL 유틸

src-tauri/src/
  commands/
    mod.rs
    meta.rs            (기존)
    connections.rs
    query.rs
    schema.rs
    ssh.rs
  db/
    mod.rs
    pool.rs
    state.rs
  ssh/
    mod.rs
    config.rs
    tunnel.rs
  secrets.rs
  errors.rs
  lib.rs              (기존, 모듈 추가)

infra/
  postgres/
    docker-compose.yml  # CI용 postgres:16-alpine

docs/
  superpowers/
    specs/
      2026-04-28-week-2-postgres-mvp-design.md  (this file)
```

## 15. Implementation slice order

writing-plans skill에서 더 잘게 쪼개지지만, 큰 흐름은:

1. **기반** — `errors.rs`, `secrets.rs`, `db::state` (rusqlite 마이그레이션) + 앱 데이터 디렉토리 셋업.
2. **DB pool + Direct TCP 연결** — `db::pool`, `commands::connections::{add, list, connect, disconnect}`, `commands::query::execute`.
3. **Frontend 연결 UI (Direct TCP)** — Form, List, store, sonner 토스트, 단순 결과 표시 (`<pre>{JSON}`).
4. **SSH config 파싱** — `ssh::config` + `commands::ssh::list_known_hosts`.
5. **SSH tunnel** — `ssh::tunnel` + `connections::add`/`connect` SSH 분기.
6. **Frontend SSH 탭들** — SshHostPicker, ConnectionForm `[alias|manual]` 탭.
7. **Schema tree** — `commands::schema` + SchemaTree 사이드바.
8. **Editor + 탭 store** — Monaco 통합, 단축키, 멀티 탭.
9. **Result grid** — TanStack + virtual + cell renderer + 헤더.
10. **마무리** — 자동 LIMIT 토글, connection-lost 이벤트, 수동 검증 체크리스트.

## 16. 결정 이력 (이번 brainstorming 세션)

- SSH 우선순위 격상 (PLAN 원안: Week 8 미루고 polish, 변경: Week 2b 필수).
  - 사유: 사용자 본인 환경(OCI + ProxyJump)이 SSH 없으면 Tusk 자체가 안 쓰임 → 매일 쓰는 도구라는 동기 정합성.
- SSH 구현은 시스템 `ssh` spawn (러스트 crate 안 씀).
  - 사유: ProxyJump/Match/Include 자동 처리, ssh-agent/1Password 자동 통합, 환경 차이 디버깅 지옥 회피.
- `~/.ssh/config` 파싱은 alias 추출만 직접 하고, effective 설정은 `ssh -G`에 위임.
  - 사유: 정확성. ssh가 직접 resolve한 결과가 진실의 원천.
- Week 2 분할: 2a (Direct TCP + 스키마 + 에디터 + grid) + 2b (SSH 1~3단계). 출시 일정 1주 연장 수용.
- 트랜잭션 컨트롤은 Week 3로 (auto-commit only).
- Cell 편집 / cancel / export 모두 Week 3.
- 프론트 단위 테스트는 Week 3에 시작.
- 자동완성은 Monaco 기본 SQL만 (스키마-aware는 Week 3+).
