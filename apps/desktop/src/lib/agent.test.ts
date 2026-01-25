import type { AgentConfigSpec } from "tauri-plugin-modular-agent-api";
import { describe, expect, it } from "vitest";

import { inferTypeForDisplay } from "./agent";

describe("inferTypeForDisplay", () => {
  it("returns explicit config type when provided", () => {
    const config = { type: "image" } as unknown as AgentConfigSpec;
    expect(inferTypeForDisplay(config, "ignored")).toBe("image");
  });

  it("infers primitives and nullish values", () => {
    expect(inferTypeForDisplay({} as AgentConfigSpec, null)).toBe("object");
    expect(inferTypeForDisplay({} as AgentConfigSpec, true)).toBe("boolean");
    expect(inferTypeForDisplay({} as AgentConfigSpec, 3)).toBe("integer");
    expect(inferTypeForDisplay({} as AgentConfigSpec, 3.14)).toBe("number");
  });

  it("detects special string formats", () => {
    expect(inferTypeForDisplay({} as AgentConfigSpec, "data:image/png;base64,abc123")).toBe(
      "image",
    );
    expect(inferTypeForDisplay({} as AgentConfigSpec, "line one\nline two")).toBe("text");
    expect(inferTypeForDisplay({} as AgentConfigSpec, "plain")).toBe("string");
  });

  it("infers message and object payloads", () => {
    expect(inferTypeForDisplay({} as AgentConfigSpec, { content: "hi" })).toBe("message");
    expect(inferTypeForDisplay({} as AgentConfigSpec, { foo: "bar" })).toBe("object");
  });

  it("infers element types in arrays", () => {
    expect(inferTypeForDisplay({} as AgentConfigSpec, [1, 2, 3])).toBe("integer");
    expect(inferTypeForDisplay({} as AgentConfigSpec, [{ content: "hello" }])).toBe("message");
    expect(inferTypeForDisplay({} as AgentConfigSpec, ["line one", "line two\nline three"])).toBe(
      "text",
    );
  });
});
