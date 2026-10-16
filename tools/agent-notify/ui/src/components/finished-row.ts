import { LitElement, css, html } from "lit";
import type { CardId } from "../bindings/CardId";
import type { FinishedEntry } from "../bindings/FinishedEntry";
import { tokens } from "../styles";

export class FinishedRow extends LitElement {
  static properties = {
    entries: { attribute: false },
    onActivate: { attribute: false },
  };

  static styles = [
    tokens,
    css`
      :host {
        display: block;
        width: var(--card-width);
      }
      .row {
        box-sizing: border-box;
        padding: 6px 14px;
        border-radius: var(--radius);
        background: var(--info-bg);
        border: 1px solid var(--info-border);
        color: var(--muted);
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
      }
      button {
        font: inherit;
        color: var(--text);
        background: none;
        border: none;
        padding: 0 3px;
        text-decoration: underline;
      }
    `,
  ];

  declare entries: readonly FinishedEntry[];
  declare onActivate: (cardId: CardId) => void;

  constructor() {
    super();
    this.entries = [];
    this.onActivate = () => {};
  }

  override render() {
    return html`<div class="row">
      Finished:
      ${this.entries.map((entry) => html`<button @click=${() => this.onActivate(entry.card_id)}>${entry.label}</button>`)}
    </div>`;
  }
}

customElements.define("finished-row", FinishedRow);
