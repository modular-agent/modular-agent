import type { ModuleConfigSpec } from "tauri-plugin-modular-agent-api";
import { describe, expect, it } from "vitest";

import { inferTypeForDisplay } from "./module";

describe("inferTypeForDisplay", () => {
  it("returns explicit config type when provided", () => {
    const config = { type: "image" } as unknown as ModuleConfigSpec;
    expect(inferTypeForDisplay(config, "ignored")).toBe("image");
  });

  it("infers primitives and nullish values", () => {
    expect(inferTypeForDisplay({} as ModuleConfigSpec, null)).toBe("null");
    expect(inferTypeForDisplay({} as ModuleConfigSpec, true)).toBe("boolean");
    expect(inferTypeForDisplay({} as ModuleConfigSpec, 3)).toBe("integer");
    expect(inferTypeForDisplay({} as ModuleConfigSpec, 3.14)).toBe("number");
  });

  it("detects special string formats", () => {
    expect(inferTypeForDisplay({} as ModuleConfigSpec, "data:image/png;base64,abc123")).toBe(
      "image",
    );
    expect(inferTypeForDisplay({} as ModuleConfigSpec, "line one\nline two")).toBe("text");
    expect(inferTypeForDisplay({} as ModuleConfigSpec, "plain")).toBe("string");
  });

  it("infers message and object payloads", () => {
    expect(inferTypeForDisplay({} as ModuleConfigSpec, { role: "user", content: "hi" })).toBe(
      "message",
    );
    expect(inferTypeForDisplay({} as ModuleConfigSpec, { type: "user", content: "hi" })).toBe(
      "message",
    );
    expect(inferTypeForDisplay({} as ModuleConfigSpec, { content: "hi" })).toBe("object");
    expect(inferTypeForDisplay({} as ModuleConfigSpec, { foo: "bar" })).toBe("object");
  });

  it("treats content-bearing objects without a role as objects", () => {
    // e.g. SearXNG search results: { title, url, content }
    expect(
      inferTypeForDisplay({} as ModuleConfigSpec, [
        { title: "Example", url: "https://example.com", content: "snippet" },
        { title: "No snippet", url: "https://example.org" },
      ]),
    ).toBe("object");
  });

  it("infers element types in arrays", () => {
    expect(inferTypeForDisplay({} as ModuleConfigSpec, [1, 2, 3])).toBe("integer");
    expect(
      inferTypeForDisplay({} as ModuleConfigSpec, [{ role: "assistant", content: "hello" }]),
    ).toBe("message");
    expect(inferTypeForDisplay({} as ModuleConfigSpec, ["line one", "line two\nline three"])).toBe(
      "text",
    );
  });
});
