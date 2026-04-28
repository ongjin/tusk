import { invoke as rawInvoke } from "@tauri-apps/api/core";

import type {
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
