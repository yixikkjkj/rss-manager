import 'yixi-lit-design/components/alert';
import 'yixi-lit-design/components/button';
import { html, nothing } from 'lit';
export const errorMessage = (message?: string) =>
  message ? html`<ld-alert class="error" theme="danger" .title=${message}></ld-alert>` : nothing;
export function pager(page: number, total: number, size: number, change: (page: number) => void) {
  return html`<div class="pager">
    <ld-button variant="default" ?disabled=${page <= 1} @ld-click=${() => change(page - 1)}>上一页</ld-button>
    <span>${page} / ${Math.max(1, Math.ceil(total / size))}（共 ${total} 篇）</span>
    <ld-button variant="default" ?disabled=${page * size >= total} @ld-click=${() => change(page + 1)}>
      下一页
    </ld-button>
  </div>`;
}
