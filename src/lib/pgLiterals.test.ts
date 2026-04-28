import { describe, it, expect } from "vitest";
import { toLiteral } from "./pgLiterals";

describe("pgLiterals.toLiteral", () => {
  it("renders Null", () => expect(toLiteral({ kind: "Null" })).toBe("NULL"));
  it("renders Bool", () =>
    expect(toLiteral({ kind: "Bool", value: true })).toBe("TRUE"));
  it("renders Text with quote escape", () =>
    expect(toLiteral({ kind: "Text", value: "o'reilly" })).toBe("'o''reilly'"));
  it("renders Uuid with cast", () =>
    expect(
      toLiteral({
        kind: "Uuid",
        value: "550e8400-e29b-41d4-a716-446655440000",
      }),
    ).toBe("'550e8400-e29b-41d4-a716-446655440000'::uuid"));
  it("renders Json with quote-escape", () =>
    expect(toLiteral({ kind: "Json", value: { k: "v's" } })).toBe(
      `'{"k":"v''s"}'::jsonb`,
    ));
  it("renders Bytea hex form", () => {
    // base64 of [0xDE,0xAD,0xBE,0xEF] = "3q2+7w=="
    expect(toLiteral({ kind: "Bytea", value: { b64: "3q2+7w==" } })).toBe(
      "'\\xdeadbeef'::bytea",
    );
  });

  it("renders Float NaN as quoted cast", () =>
    expect(toLiteral({ kind: "Float", value: Number.NaN })).toBe(
      "'NaN'::float8",
    ));

  it("renders Float Infinity as quoted cast", () => {
    expect(toLiteral({ kind: "Float", value: Number.POSITIVE_INFINITY })).toBe(
      "'Infinity'::float8",
    );
    expect(toLiteral({ kind: "Float", value: Number.NEGATIVE_INFINITY })).toBe(
      "'-Infinity'::float8",
    );
  });
});
