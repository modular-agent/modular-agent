import type { AgentDisplayConfigEntry } from "tauri-plugin-askit-api";
import { describe, expect, it } from "vitest";

import { inferTypeForDisplay } from "./agent";

describe("inferTypeForDisplay", () => {
  it("returns explicit config type when provided", () => {
    const config = { type: "image" } as unknown as AgentDisplayConfigEntry;
    expect(inferTypeForDisplay(config, "ignored")).toBe("image");
  });

  it("infers primitives and nullish values", () => {
    expect(inferTypeForDisplay({} as AgentDisplayConfigEntry, null)).toBe("object");
    expect(inferTypeForDisplay({} as AgentDisplayConfigEntry, true)).toBe("boolean");
    expect(inferTypeForDisplay({} as AgentDisplayConfigEntry, 3)).toBe("integer");
    expect(inferTypeForDisplay({} as AgentDisplayConfigEntry, 3.14)).toBe("number");
  });

  it("detects special string formats", () => {
    expect(inferTypeForDisplay({} as AgentDisplayConfigEntry, "data:image/png;base64,abc123")).toBe(
      "image",
    );
    expect(inferTypeForDisplay({} as AgentDisplayConfigEntry, "line one\nline two")).toBe("text");
    expect(inferTypeForDisplay({} as AgentDisplayConfigEntry, "plain")).toBe("string");
  });

  it("infers message and object payloads", () => {
    expect(inferTypeForDisplay({} as AgentDisplayConfigEntry, { content: "hi" })).toBe("message");
    expect(inferTypeForDisplay({} as AgentDisplayConfigEntry, { foo: "bar" })).toBe("object");
  });

  it("infers element types in arrays", () => {
    expect(inferTypeForDisplay({} as AgentDisplayConfigEntry, [1, 2, 3])).toBe("integer");
    expect(inferTypeForDisplay({} as AgentDisplayConfigEntry, [{ content: "hello" }])).toBe(
      "message",
    );
    expect(
      inferTypeForDisplay({} as AgentDisplayConfigEntry, ["line one", "line two\nline three"]),
    ).toBe("text");
  });
});
