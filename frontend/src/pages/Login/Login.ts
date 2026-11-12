import 'yixi-lit-design/components/button';
import 'yixi-lit-design/components/input';
import { html } from 'lit';
import { AppElement } from '../../core/AppElement';
import { navigate } from '../../core/Router';
import { Action } from '../../controllers/Action';
import { errorMessage } from '../../components/Feedback/Feedback';
import { api } from '../../Api';
import { auth } from '../../Store';

export class Login extends AppElement {
  static properties = { username: { state: true }, password: { state: true } };
  declare private username: string;
  declare private password: string;
  private action = new Action(this);
  constructor() {
    super();
    this.username = 'admin';
    this.password = '';
  }
  private submit = (event: Event) => {
    event.preventDefault();
    void this.action.run(async (current) => {
      const user = await api.authLogin(this.username, this.password);
      if (!current()) return;
      auth.setUser(user);
      navigate('/', { replace: true });
    });
  };
  render() {
    return html`<div class="card">
      <h1>登录 rss-manager</h1>
      <form @submit=${this.submit}>
        <label
          >用户名<ld-input
            name="username"
            .value=${this.username}
            @ld-input=${(e: CustomEvent<{ value: string }>) => {
              this.username = e.detail.value;
            }}
            autocomplete="username"
            required
          ></ld-input
        ></label>
        <label
          >密码<ld-input
            name="password"
            type="password"
            .value=${this.password}
            @ld-input=${(e: CustomEvent<{ value: string }>) => {
              this.password = e.detail.value;
            }}
            autocomplete="current-password"
            required
          ></ld-input
        ></label>
        ${errorMessage(this.action.error)}
        <ld-button variant="primary" type="submit" ?disabled=${this.action.busy} ?loading=${this.action.busy}
          ><span>${this.action.busy ? '登录中…' : '登录'}</span></ld-button
        >
      </form>
    </div>`;
  }
}
customElements.define('rss-login', Login);
