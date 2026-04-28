export type SshKind = "None" | "Alias" | "Manual";

export interface ConnectionRecord {
  id: string;
  name: string;
  host: string;
  port: number;
  dbUser: string;
  database: string;
  sslMode: string;
  sshKind: SshKind;
  sshAlias: string | null;
  sshHost: string | null;
  sshPort: number | null;
  sshUser: string | null;
  sshKeyPath: string | null;
  createdAt: number;
  updatedAt: number;
}

export interface NewConnection {
  name: string;
  host: string;
  port: number;
  dbUser: string;
  database: string;
  sslMode: string;
  sshKind: SshKind;
  sshAlias: string | null;
  sshHost: string | null;
  sshPort: number | null;
  sshUser: string | null;
  sshKeyPath: string | null;
}

export interface ConnectionListItem extends ConnectionRecord {
  connected: boolean;
}

export interface ColumnMeta {
  name: string;
  oid: number;
  typeName: string;
}

export interface QueryResult {
  columns: ColumnMeta[];
  rows: Cell[][];
  durationMs: number;
  rowCount: number;
  meta: ResultMeta;
}

export interface TuskErrorPayload {
  kind:
    | "Connection"
    | "Query"
    | "Tunnel"
    | "Ssh"
    | "State"
    | "Secrets"
    | "Internal"
    | "Editing"
    | "Conflict"
    | "Tx"
    | "TxAborted"
    | "QueryCancelled"
    | "History"
    | "UnsupportedEditType";
  data?: unknown;
}

export class TuskError extends Error {
  kind: TuskErrorPayload["kind"];
  data: unknown;
  constructor(payload: TuskErrorPayload) {
    const msg =
      typeof payload.data === "string"
        ? payload.data
        : (payload.data ?? payload.kind);
    super(typeof msg === "string" ? msg : JSON.stringify(msg));
    this.kind = payload.kind;
    this.data = payload.data;
    this.name = `TuskError(${payload.kind})`;
  }
}

export interface SshHost {
  alias: string;
  hostname: string | null;
  user: string | null;
  port: number | null;
  identityFile: string | null;
  proxyJump: string | null;
}

export interface ColumnInfo {
  name: string;
  data_type: string;
  is_nullable: boolean;
}

export type PgTypeName =
  | "bool"
  | "int2"
  | "int4"
  | "int8"
  | "float4"
  | "float8"
  | "numeric"
  | "text"
  | "varchar"
  | "bpchar"
  | "bytea"
  | "uuid"
  | "inet"
  | "cidr"
  | "date"
  | "time"
  | "timetz"
  | "timestamp"
  | "timestamptz"
  | "interval"
  | "jsonb"
  | "json"
  | "enum"
  | "vector"
  | "unknown";

export type Cell =
  | { kind: "Null" }
  | { kind: "Bool"; value: boolean }
  | { kind: "Int"; value: number }
  | { kind: "Bigint"; value: string }
  | { kind: "Float"; value: number }
  | { kind: "Numeric"; value: string }
  | { kind: "Text"; value: string }
  | { kind: "Bytea"; value: { b64: string } }
  | { kind: "Uuid"; value: string }
  | { kind: "Inet"; value: string }
  | { kind: "Date"; value: string }
  | { kind: "Time"; value: string }
  | { kind: "Timetz"; value: string }
  | { kind: "Timestamp"; value: string }
  | { kind: "Timestamptz"; value: string }
  | { kind: "Interval"; value: { iso: string } }
  | { kind: "Json"; value: unknown }
  | { kind: "Array"; value: { elem: string; values: Cell[] } }
  | { kind: "Enum"; value: { typeName: string; value: string } }
  | { kind: "Vector"; value: { dim: number; values: number[] } }
  | { kind: "Unknown"; value: { oid: number; text: string } };

export interface ColumnTypeMeta {
  name: string;
  oid: number;
  typeName: PgTypeName;
  nullable: boolean;
  enumValues?: string[];
  fk?: { schema: string; table: string; column: string };
}

export interface ResultMeta {
  editable: boolean;
  reason?:
    | "no-pk"
    | "multi-table"
    | "computed"
    | "pk-not-in-select"
    | "too-large"
    | "parser-failed"
    | "unknown-type";
  table?: { schema: string; name: string };
  pkColumns: string[];
  pkColumnIndices: number[];
  columnTypes: ColumnTypeMeta[];
}

export interface TxState {
  connId: string;
  active: boolean;
  txId?: string;
  startedAt?: number;
  statementCount: number;
  lastError?: string;
  pid?: number;
}

export interface HistoryEntry {
  id: string;
  connId: string;
  source: "editor" | "inline" | "palette";
  txId?: string;
  sqlPreview: string;
  sqlFull?: string;
  startedAt: number;
  durationMs: number;
  rowCount?: number;
  status: "ok" | "error" | "cancelled" | "rolled_back" | "open";
  errorMessage?: string;
  statementCount: number;
}

export interface HistoryStatement {
  id: string;
  entryId: string;
  ordinal: number;
  sql: string;
  durationMs: number;
  rowCount?: number;
  status: "ok" | "error";
  errorMessage?: string;
}

export interface PendingChange {
  rowKey: string;
  table: { schema: string; name: string };
  pk: { columns: string[]; values: Cell[] };
  edits: { column: string; original: Cell; next: Cell }[];
  op: "update" | "insert" | "delete";
  capturedRow: Cell[];
  capturedColumns: string[];
  capturedAt: number;
}

export type AiProvider = "openai" | "anthropic" | "gemini" | "ollama";

export interface ProviderConfig {
  provider: AiProvider;
  apiKeyPresent: boolean;
  baseUrl?: string;
  generationModel: string;
  embeddingModel?: string;
}

export type DestructiveKind =
  | "drop-database"
  | "drop-schema"
  | "drop-table"
  | "drop-column"
  | "drop-index"
  | "drop-view"
  | "drop-function"
  | "truncate"
  | "delete-no-where"
  | "update-no-where"
  | "alter-drop-constraint"
  | "grant-revoke-all"
  | "vacuum-full"
  | "parser-failed";

export interface DestructiveFinding {
  kind: DestructiveKind;
  statementIndex: number;
  message: string;
  affectedObject?: string;
}

export interface SchemaIndexProgress {
  connId: string;
  state: "idle" | "running" | "done" | "error";
  totalTables: number;
  embeddedTables: number;
  errorMessage?: string;
  lastSyncedAt?: number;
}

export interface AiHistoryMeta {
  source: "ai";
  provider: AiProvider;
  generationModel: string;
  embeddingModel?: string;
  prompt: string;
  generatedSql: string;
  topKTables: string[];
  toolCalls: { name: string; args: unknown }[];
  promptTokens?: number;
  completionTokens?: number;
}
