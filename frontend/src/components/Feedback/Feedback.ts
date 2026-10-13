import { html, nothing } from 'lit';
export const errorMessage = (message?: string) =>
  message ? html`<p class="error" role="alert">${message}</p>` : nothing;
export function pager(page: number, total: number, size: number, change: (page: number) => void) {
  return html`<div class="pager">
    <button class="secondary" ?disabled=${page <= 1} @click=${() => change(page - 1)}>上一页</button>
    <span>${page} / ${Math.max(1, Math.ceil(total / size))}（共 ${total} 篇）</span>
    <button class="secondary" ?disabled=${page * size >= total} @click=${() => change(page + 1)}>
      下一页
    </button>
  </div>`;
}
