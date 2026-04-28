import { invoke } from "@tauri-apps/api/core";
import { tool } from "ai";
import { z } from "zod";

interface ToolDeps {
  connectionId: string;
  sampleRowsEnabled: boolean;
}

export function buildTools(deps: ToolDeps) {
  const base = {
    get_table_schema: tool({
      description:
        "Fetch the CREATE TABLE DDL for a specific schema-qualified table.",
      inputSchema: z.object({ schema: z.string(), table: z.string() }),
      execute: async ({ schema, table }: { schema: string; table: string }) =>
        invoke<string>("get_table_schema", {
          connectionId: deps.connectionId,
          schema,
          table,
        }),
    }),
    list_indexes: tool({
      description:
        "List indexes (name, columns, type) defined on a specific table.",
      inputSchema: z.object({ schema: z.string(), table: z.string() }),
      execute: async ({ schema, table }: { schema: string; table: string }) =>
        invoke<unknown>("list_indexes", {
          connectionId: deps.connectionId,
          schema,
          table,
        }),
    }),
  };
  if (!deps.sampleRowsEnabled) return base;
  return {
    ...base,
    sample_rows: tool({
      description:
        "Sample up to N rows. Use sparingly — rows leave the device.",
      inputSchema: z.object({
        schema: z.string(),
        table: z.string(),
        limit: z.number().int().min(1).max(20),
      }),
      execute: async ({
        schema,
        table,
        limit,
      }: {
        schema: string;
        table: string;
        limit: number;
      }) =>
        invoke<unknown>("sample_rows", {
          connectionId: deps.connectionId,
          schema,
          table,
          limit,
        }),
    }),
  };
}
