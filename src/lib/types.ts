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
  type_name: string;
}

export interface QueryResult {
  columns: ColumnMeta[];
  rows: Array<Array<unknown>>;
  durationMs: number;
  rowCount: number;
}

export interface TuskErrorPayload {
  kind:
    | "Connection"
    | "Query"
    | "Tunnel"
    | "Ssh"
    | "State"
    | "Secrets"
    | "Internal";
  message: string;
}

export class TuskError extends Error {
  kind: TuskErrorPayload["kind"];
  constructor(payload: TuskErrorPayload) {
    super(payload.message);
    this.kind = payload.kind;
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
