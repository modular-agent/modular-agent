export type ParsedConfigInput = { ok: true; value: unknown } | { ok: false; message: string };

const INTEGER_RE = /^-?\d+$/;
const SAFE_RANGE = `±${Number.MAX_SAFE_INTEGER}`;

// Why a parsed JSON value would be stored as something else, or null if it
// survives: JSON.parse turns 1e400 into Infinity, which serializes as null, and
// rounds integers beyond 2^53 - 1. After parsing, a rounded integer cannot be
// told apart from a float written as 1e100, so both are refused.
function lossyNumber(value: unknown): string | null {
  if (typeof value === "number") {
    if (!Number.isFinite(value)) return "contains a number too large to store";
    if (Number.isInteger(value) && !Number.isSafeInteger(value)) {
      return `contains an integer outside ${SAFE_RANGE}`;
    }
    return null;
  }
  if (value !== null && typeof value === "object") {
    for (const item of Object.values(value)) {
      const reason = lossyNumber(item);
      if (reason) return reason;
    }
  }
  return null;
}

/**
 * Converts the text of a settings field to the value stored for a config of
 * `type`. Anything that would be stored as a different value than the one
 * typed is rejected instead of being saved.
 */
export function parseConfigInput(
  type: "integer" | "number" | "object" | "array",
  text: string,
): ParsedConfigInput {
  const trimmed = text.trim();

  if (type === "integer") {
    // Decimal notation only: Number() would also take "1e100" and "0x10", and
    // the core reads the value as an i64.
    if (!INTEGER_RE.test(trimmed)) return { ok: false, message: "Enter a whole number" };
    const value = Number(trimmed);
    if (!Number.isSafeInteger(value)) {
      return { ok: false, message: `Enter a whole number within ${SAFE_RANGE}` };
    }
    return { ok: true, value };
  }

  if (type === "number") {
    const value = Number(trimmed);
    if (trimmed === "" || !Number.isFinite(value)) {
      return { ok: false, message: "Enter a number" };
    }
    return { ok: true, value };
  }

  let value: unknown;
  try {
    value = JSON.parse(text);
  } catch (e) {
    return { ok: false, message: `Invalid JSON: ${e instanceof Error ? e.message : e}` };
  }
  const isArray = Array.isArray(value);
  if (type === "array" ? !isArray : isArray || value === null || typeof value !== "object") {
    return { ok: false, message: `Enter a JSON ${type}` };
  }
  const reason = lossyNumber(value);
  if (reason) return { ok: false, message: `The ${type} ${reason}` };
  return { ok: true, value };
}
