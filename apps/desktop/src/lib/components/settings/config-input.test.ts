import { describe, expect, it } from "vitest";

import { parseConfigInput } from "./config-input";

const ok = (value: unknown) => ({ ok: true, value });
const rejected = { ok: false, message: expect.any(String) };

describe("parseConfigInput: integer", () => {
  it("returns a number, not the typed string", () => {
    expect(parseConfigInput("integer", "8080")).toEqual(ok(8080));
    expect(parseConfigInput("integer", " -42 ")).toEqual(ok(-42));
    expect(parseConfigInput("integer", "0")).toEqual(ok(0));
  });

  it("accepts the safe integer bounds", () => {
    expect(parseConfigInput("integer", "9007199254740991")).toEqual(ok(9007199254740991));
    expect(parseConfigInput("integer", "-9007199254740991")).toEqual(ok(-9007199254740991));
  });

  it("rejects integers that would be rounded", () => {
    expect(parseConfigInput("integer", "9007199254740992")).toEqual(rejected);
    expect(parseConfigInput("integer", "9007199254740993")).toEqual(rejected);
    expect(parseConfigInput("integer", "-9007199254740992")).toEqual(rejected);
  });

  it("rejects anything but decimal integer notation", () => {
    for (const text of ["1e100", "1e3", "1.5", "1.0", "0x10", "+1", "", "  ", "abc", "${PORT}"]) {
      expect(parseConfigInput("integer", text), text).toEqual(rejected);
    }
  });
});

describe("parseConfigInput: number", () => {
  it("accepts finite numbers", () => {
    expect(parseConfigInput("number", "1.5")).toEqual(ok(1.5));
    expect(parseConfigInput("number", "-0.25")).toEqual(ok(-0.25));
    expect(parseConfigInput("number", "1e3")).toEqual(ok(1000));
    expect(parseConfigInput("number", "3")).toEqual(ok(3));
  });

  it("rejects empty, non-numeric and non-finite input", () => {
    for (const text of ["", "  ", "abc", "1e400", "-1e400", "Infinity", "NaN"]) {
      expect(parseConfigInput("number", text), text).toEqual(rejected);
    }
  });
});

describe("parseConfigInput: object / array", () => {
  it("returns the parsed value", () => {
    expect(parseConfigInput("object", '{"a": 1, "b": "${TOKEN}"}')).toEqual(
      ok({ a: 1, b: "${TOKEN}" }),
    );
    expect(parseConfigInput("object", "{}")).toEqual(ok({}));
    expect(parseConfigInput("array", '[1, "two", null]')).toEqual(ok([1, "two", null]));
    expect(parseConfigInput("array", "[]")).toEqual(ok([]));
  });

  it("keeps valid nested values as written", () => {
    const nested = { a: { b: [1, 2.5, { c: 9007199254740991, d: null, e: [true, "x"] }] } };
    expect(parseConfigInput("object", JSON.stringify(nested))).toEqual(ok(nested));
    expect(parseConfigInput("array", JSON.stringify([nested, [nested]]))).toEqual(
      ok([nested, [nested]]),
    );
  });

  it("rejects invalid JSON", () => {
    expect(parseConfigInput("object", '{"a": 1')).toEqual(rejected);
    expect(parseConfigInput("object", "")).toEqual(rejected);
    expect(parseConfigInput("array", "[1,]")).toEqual(rejected);
  });

  it("rejects a value of the wrong shape", () => {
    for (const text of ["[1]", '"text"', "1", "true", "null"]) {
      expect(parseConfigInput("object", text), text).toEqual(rejected);
    }
    for (const text of ['{"a": 1}', '"text"', "1", "true", "null"]) {
      expect(parseConfigInput("array", text), text).toEqual(rejected);
    }
  });

  it("rejects non-finite numbers at any depth", () => {
    for (const text of ['{"x": 1e400}', '{"x": -1e400}', '{"a": {"b": [1, {"c": 1e400}]}}']) {
      expect(parseConfigInput("object", text), text).toEqual(rejected);
    }
    for (const text of ["[1e400]", "[[1, [2, 1e400]]]", '[{"a": [{"b": 1e400}]}]']) {
      expect(parseConfigInput("array", text), text).toEqual(rejected);
    }
  });

  it("rejects integers that were rounded at any depth", () => {
    for (const text of [
      '{"x": 9007199254740993}',
      '{"a": {"b": [1, {"c": -9007199254740993}]}}',
      '{"x": 1e100}',
    ]) {
      expect(parseConfigInput("object", text), text).toEqual(rejected);
    }
    for (const text of ["[9007199254740993]", "[[1, [2, 9007199254740992]]]"]) {
      expect(parseConfigInput("array", text), text).toEqual(rejected);
    }
  });
});
