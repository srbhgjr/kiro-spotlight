import DOMPurify from "dompurify";
import { Marked } from "marked";

const marked = new Marked({ gfm: true, breaks: false, async: false });

/** Render agent Markdown to sanitised HTML. */
export function renderMarkdown(text: string): string {
  const html = marked.parse(text) as string;
  return DOMPurify.sanitize(html, {
    USE_PROFILES: { html: true },
    FORBID_TAGS: ["style", "form", "input", "button", "iframe"],
  });
}

/** The http(s) URL of a clicked link inside rendered Markdown, if any. */
export function linkTarget(event: MouseEvent): string | null {
  const anchor = (event.target as HTMLElement | null)?.closest("a");
  const href = anchor?.getAttribute("href") ?? "";
  return /^https?:\/\//i.test(href) ? href : null;
}
