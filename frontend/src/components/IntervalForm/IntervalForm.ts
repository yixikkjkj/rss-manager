import 'yixi-lit-design/components/button';
import 'yixi-lit-design/components/input';
import { html, nothing } from 'lit';
import { AppElement } from '../../core/AppElement';
import { Action } from '../../controllers/Action';
import { errorMessage } from '../Feedback/Feedback';
import { api } from '../../Api';

class IntervalForm extends AppElement {
  static properties = { seconds: { type: Number }, minutes: { state: true }, saved: { state: true } };
  declare seconds: number;
  declare private minutes: string;
  declare private saved: boolean;
  private action = new Action(this);
  constructor() {
    super();
    this.minutes = '';
    this.saved = false;
  }
  connectedCallback() {
    super.connectedCallback();
    this.minutes = String(this.seconds / 60);
  }
  render() {
    return html`<form
      class="panel"
      @submit=${(e: Event) => {
        e.preventDefault();
        void this.action.run(async (current) => {
          await api.updateFetchConfig(Number(this.minutes) * 60);
          if (current()) this.saved = true;
        });
      }}
    >
      <h2>默认抓取间隔</h2>
      <label
        >间隔（分钟）<ld-input
          name="fetch_interval_minutes"
          type="number"
          min="30"
          max="10080"
          step="1"
          required
          .value=${this.minutes}
          @ld-input=${(e: CustomEvent<{ value: string }>) => {
            this.minutes = e.detail.value;
            this.saved = false;
          }}
        ></ld-input
      ></label>
      <p class="meta">适用于选择“继承全局”的源。单源可在左侧文件树的订阅设置中覆盖此值，最低 30 分钟。</p>
      <ld-button variant="primary" type="submit" ?disabled=${this.action.busy} ?loading=${this.action.busy}
        >保存间隔</ld-button
      >${this.saved ? html`<span role="status"> 已保存</span>` : nothing} ${errorMessage(this.action.error)}
    </form>`;
  }
}
customElements.define('rss-interval-form', IntervalForm);
