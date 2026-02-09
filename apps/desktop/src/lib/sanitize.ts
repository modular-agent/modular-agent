import DOMPurify from "dompurify";
import { marked } from "marked";

const PURIFY_CONFIG = {
  ALLOWED_TAGS: [
    "p",
    "br",
    "strong",
    "em",
    "b",
    "i",
    "u",
    "s",
    "del",
    "code",
    "pre",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "ul",
    "ol",
    "li",
    "a",
    "blockquote",
    "hr",
    "table",
    "thead",
    "tbody",
    "tr",
    "th",
    "td",
    "details",
    "summary",
    "sup",
    "sub",
    "img",
    "div",
    "span",
  ],
  ALLOWED_ATTR: ["href", "src", "alt", "class", "open", "target", "rel"],
  ALLOW_DATA_ATTR: false,
};

export function sanitizeHtml(html: string): string {
  return DOMPurify.sanitize(html, PURIFY_CONFIG);
}

export function renderMarkdown(raw: string): string {
  return DOMPurify.sanitize(marked.parse(raw) as string, PURIFY_CONFIG);
}

export function escapeHtml(str: string): string {
  return str
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#039;");
}

export function isSafeImageSrc(src: string): boolean {
  if (typeof src !== "string") return false;
  return (
    src.startsWith("data:image/") || src.startsWith("https://") || src.startsWith("http://")
  );
}
