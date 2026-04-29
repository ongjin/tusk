import { describe, expect, it } from "vitest";

import { buildAnnSql } from "./annSql";

describe("buildAnnSql", () => {
  it("single PK + cosine", () => {
    const sql = buildAnnSql({
      schema: "public",
      table: "items",
      vecCol: "embedding",
      pkCols: ["id"],
      queryVector: [0.1, 0.2, 0.3],
      op: "<=>",
      limit: 20,
    });
    expect(sql).toContain('"public"."items"');
    expect(sql).toContain('"embedding" <=> \'[0.1,0.2,0.3]\'::vector AS distance');
    expect(sql).toMatch(/SELECT "id",/);
    expect(sql).toContain("ORDER BY distance");
    expect(sql).toContain("LIMIT 20");
  });

  it("composite PK selects both columns", () => {
    const sql = buildAnnSql({
      schema: "public",
      table: "items",
      vecCol: "embedding",
      pkCols: ["tenant", "id"],
      queryVector: [1, 2],
      op: "<->",
      limit: 5,
    });
    expect(sql).toMatch(/SELECT "tenant", "id",/);
    expect(sql).toContain("<->");
  });

  it("supports inner-product operator", () => {
    const sql = buildAnnSql({
      schema: "public",
      table: "items",
      vecCol: "embedding",
      pkCols: ["id"],
      queryVector: [0],
      op: "<#>",
      limit: 10,
    });
    expect(sql).toContain("<#>");
  });

  it("escapes identifiers with quotes/uppercase", () => {
    const sql = buildAnnSql({
      schema: 'pub"lic',
      table: "Items",
      vecCol: "Embedding",
      pkCols: ['I"d'],
      queryVector: [0],
      op: "<=>",
      limit: 1,
    });
    expect(sql).toContain('"pub""lic"."Items"');
    expect(sql).toContain('"Embedding"');
    expect(sql).toContain('"I""d"');
  });

  it("clamps limit to [1, 10000]", () => {
    expect(
      buildAnnSql({
        schema: "s",
        table: "t",
        vecCol: "v",
        pkCols: ["id"],
        queryVector: [0],
        op: "<=>",
        limit: -5,
      }),
    ).toContain("LIMIT 1");
    expect(
      buildAnnSql({
        schema: "s",
        table: "t",
        vecCol: "v",
        pkCols: ["id"],
        queryVector: [0],
        op: "<=>",
        limit: 99999,
      }),
    ).toContain("LIMIT 10000");
  });
});
