import type { AgentDisplayConfigSpec } from "tauri-plugin-askit-api";
import { describe, expect, it } from "vitest";

import { inferTypeForDisplay } from "./agent";

describe("inferTypeForDisplay", () => {
  it("returns explicit config type when provided", () => {
    const config = { type: "image" } as unknown as AgentDisplayConfigSpec;
    expect(inferTypeForDisplay(config, "ignored")).toBe("image");
  });

  it("infers primitives and nullish values", () => {
    expect(inferTypeForDisplay({} as AgentDisplayConfigSpec, null)).toBe("object");
    expect(inferTypeForDisplay({} as AgentDisplayConfigSpec, true)).toBe("boolean");
    expect(inferTypeForDisplay({} as AgentDisplayConfigSpec, 3)).toBe("integer");
    expect(inferTypeForDisplay({} as AgentDisplayConfigSpec, 3.14)).toBe("number");
  });

  it("detects special string formats", () => {
    expect(inferTypeForDisplay({} as AgentDisplayConfigSpec, "data:image/png;base64,abc123")).toBe(
      "image",
    );
    expect(inferTypeForDisplay({} as AgentDisplayConfigSpec, "line one\nline two")).toBe("text");
    expect(inferTypeForDisplay({} as AgentDisplayConfigSpec, "plain")).toBe("string");
  });

  it("infers message and object payloads", () => {
    expect(inferTypeForDisplay({} as AgentDisplayConfigSpec, { content: "hi" })).toBe("message");
    expect(inferTypeForDisplay({} as AgentDisplayConfigSpec, { foo: "bar" })).toBe("object");
  });

  it("infers element types in arrays", () => {
    expect(inferTypeForDisplay({} as AgentDisplayConfigSpec, [1, 2, 3])).toBe("integer");
    expect(inferTypeForDisplay({} as AgentDisplayConfigSpec, [{ content: "hello" }])).toBe(
      "message",
    );
    expect(
      inferTypeForDisplay({} as AgentDisplayConfigSpec, ["line one", "line two\nline three"]),
    ).toBe("text");
  });
});
