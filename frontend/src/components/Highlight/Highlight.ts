import { html } from 'lit';
export function highlight(text: string, query: string) {
  const terms = query.toLowerCase().split(/\s+/).filter(Boolean);
  const parts: { text: string; marked: boolean }[] = [];
  const lower = text.toLowerCase();
  let offset = 0;
  while (offset < text.length) {
    let start = text.length,
      length = 0;
    for (const term of terms) {
      const position = lower.indexOf(term, offset);
      if (position >= 0 && position < start) {
        start = position;
        length = term.length;
      }
    }
    if (start > offset) parts.push({ text: text.slice(offset, start), marked: false });
    if (!length) break;
    parts.push({ text: text.slice(start, start + length), marked: true });
    offset = start + length;
  }
  return parts.map((part) => (part.marked ? html`<mark>${part.text}</mark>` : part.text));
}
