import type { AnnOperator } from "./types";

export interface BuildAnnSqlArgs {
  schema: string;
  table: string;
  vecCol: string;
  pkCols: string[];
  queryVector: number[];
  op: AnnOperator;
  limit: number;
}

export function buildAnnSql(args: BuildAnnSqlArgs): string {
  const limit = Math.max(1, Math.min(10_000, Math.floor(args.limit)));
  const pkSelect = args.pkCols.map(escIdent).join(", ");
  const vecLit = `'[${args.queryVector.join(",")}]'::vector`;
  return [
    `SELECT ${pkSelect},`,
    `       ${escIdent(args.vecCol)} ${args.op} ${vecLit} AS distance,`,
    `       *`,
    `FROM ${escIdent(args.schema)}.${escIdent(args.table)}`,
    `ORDER BY distance`,
    `LIMIT ${limit};`,
  ].join("\n");
}

function escIdent(s: string): string {
  return `"${s.replace(/"/g, '""')}"`;
}
