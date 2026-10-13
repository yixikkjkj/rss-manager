import DOMPurify from 'dompurify';
import { html } from 'lit';
import { unsafeHTML } from 'lit/directives/unsafe-html.js';
import styles from './SafeHtml.module.scss';

export function safeHref(raw: string): string | undefined {
  try {
    const url = new URL(raw);
    return ['https:', 'http:'].includes(url.protocol) ? url.href : undefined;
  } catch {
    return undefined;
  }
}
export function sanitizeHtml(value: string): string {
  return DOMPurify.sanitize(value, {
    USE_PROFILES: { html: true },
    FORBID_TAGS: ['style', 'form', 'input', 'button', 'textarea', 'select', 'iframe'],
    FORBID_ATTR: ['style'],
    ALLOWED_URI_REGEXP: /^https?:\/\//i,
  });
}
/** Sole external-HTML sink. Keep sanitization and insertion together. */
export const safeHtml = (value: string) =>
  html`<div class=${styles.content} data-testid="article-content">${unsafeHTML(sanitizeHtml(value))}</div>`;
