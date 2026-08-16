// Message.content wire shapes produced by modular-agent-core:
// (a) legacy plain string (optionally with a top-level "thinking" key),
// (b) plain string for new pure-text messages, and
// (c) an array of internally tagged content blocks ({"type": "text" | "thinking" | "image"}).
// Field names mirror the core's serde representation exactly.

export type ContentBlock =
  | { type: "text"; text: string }
  | { type: "thinking"; thinking: string; signature?: string; redacted?: boolean }
  | { type: "image"; data: string; mime_type: string };

// Legacy histories may also carry string arrays, so items are handled per-element.
export type MessageContent = string | (string | ContentBlock)[];

function isBlock(item: string | ContentBlock): item is ContentBlock {
  return typeof item === "object" && item !== null;
}

/** Main text of a message: the string itself, or text blocks/strings joined. */
export function extractText(content: MessageContent | undefined): string {
  if (typeof content === "string") {
    return content;
  }
  if (!Array.isArray(content)) {
    return "";
  }
  return content
    .map((item) => {
      if (!isBlock(item)) {
        return item;
      }
      return item.type === "text" ? item.text : "";
    })
    .filter((s) => s !== "")
    .join("\n\n");
}

/** Thinking trace from content blocks; redacted blocks become "[redacted]". */
export function extractThinking(content: MessageContent | undefined): string | undefined {
  if (!Array.isArray(content)) {
    return undefined;
  }
  const parts = content
    .filter(isBlock)
    .filter((b): b is Extract<ContentBlock, { type: "thinking" }> => b.type === "thinking")
    .map((b) => (b.redacted ? "[redacted]" : b.thinking));
  return parts.length > 0 ? parts.join("\n\n") : undefined;
}

/** Image block sources as data URIs; null for blocks missing data or mime_type. */
export function extractImageSrcs(content: MessageContent | undefined): (string | null)[] {
  if (!Array.isArray(content)) {
    return [];
  }
  return content
    .filter(isBlock)
    .filter((b): b is Extract<ContentBlock, { type: "image" }> => b.type === "image")
    .map((b) => (b.data && b.mime_type ? `data:${b.mime_type};base64,${b.data}` : null));
}
