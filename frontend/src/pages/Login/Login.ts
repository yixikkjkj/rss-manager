import { html } from 'lit';
import { AppElement, valueOf } from '../../core/AppElement';
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
          >用户名<input
            .value=${this.username}
            @input=${(e: Event) => {
              this.username = valueOf(e);
            }}
            autocomplete="username"
            required
        /></label>
        <label
          >密码<input
            type="password"
            .value=${this.password}
            @input=${(e: Event) => {
              this.password = valueOf(e);
            }}
            autocomplete="current-password"
            required
        /></label>
        ${errorMessage(this.action.error)}
        <button type="submit" ?disabled=${this.action.busy}>${this.action.busy ? '登录中…' : '登录'}</button>
      </form>
    </div>`;
  }
}
customElements.define('rss-login', Login);
