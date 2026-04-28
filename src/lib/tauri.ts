import { invoke as rawInvoke } from "@tauri-apps/api/core";

import type {
  ColumnInfo,
  ConnectionListItem,
  ConnectionRecord,
  NewConnection,
  QueryResult,
  SshHost,
  TuskErrorPayload,
} from "./types";
import { TuskError } from "./types";

async function invoke<T>(
  cmd: string,
  args?: Record<string, unknown>,
): Promise<T> {
  try {
    return await rawInvoke<T>(cmd, args);
  } catch (e) {
    if (e && typeof e === "object" && "kind" in e && "message" in e) {
      throw new TuskError(e as TuskErrorPayload);
    }
    throw e;
  }
}

export async function greet(name: string): Promise<string> {
  return invoke<string>("greet", { name });
}

export async function listConnections(): Promise<ConnectionListItem[]> {
  return invoke<ConnectionListItem[]>("list_connections");
}

export async function addConnection(
  newConnection: NewConnection,
  password: string,
): Promise<ConnectionRecord> {
  return invoke<ConnectionRecord>("add_connection", {
    new: newConnection,
    password,
  });
}

export async function deleteConnection(id: string): Promise<void> {
  return invoke<void>("delete_connection", { id });
}

export async function connect(id: string): Promise<void> {
  return invoke<void>("connect", { id });
}

export async function disconnect(id: string): Promise<void> {
  return invoke<void>("disconnect", { id });
}

export async function executeQuery(
  connectionId: string,
  sql: string,
): Promise<QueryResult> {
  return invoke<QueryResult>("execute_query", { connectionId, sql });
}

export async function listKnownSshHosts(): Promise<SshHost[]> {
  return invoke<SshHost[]>("list_known_ssh_hosts");
}

export async function listDatabases(connectionId: string) {
  return invoke<string[]>("list_databases", { connectionId });
}
export async function listSchemas(connectionId: string) {
  return invoke<string[]>("list_schemas", { connectionId });
}
export async function listTables(connectionId: string, schema: string) {
  return invoke<string[]>("list_tables", { connectionId, schema });
}
export async function listColumns(
  connectionId: string,
  schema: string,
  table: string,
) {
  return invoke<ColumnInfo[]>("list_columns", { connectionId, schema, table });
}
