import { LitElement } from 'lit';

/** Application components share document styles. Opt into LitElement for shadow isolation. */
export abstract class AppElement extends LitElement {
  protected override createRenderRoot() {
    return this;
  }
}

export const valueOf = (event: Event) => (event.currentTarget as HTMLInputElement).value;
export const checkedOf = (event: Event) => (event.currentTarget as HTMLInputElement).checked;
