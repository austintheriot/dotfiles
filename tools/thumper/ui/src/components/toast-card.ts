import { LitElement, css, html, nothing } from "lit";
import type { CardId } from "../bindings/CardId";
import type { CardView } from "../bindings/CardView";
import { tokens } from "../styles";
import { relativeTime } from "../time";

export class ToastCard extends LitElement {
  static properties = {
    card: { attribute: false },
    now: { type: Number },
    closeVisible: { type: Boolean, reflect: true, attribute: "close-visible" },
    nativeHover: { type: Boolean, reflect: true, attribute: "native-hover" },
    onActivate: { attribute: false },
    onClose: { attribute: false },
  };

  static styles = [
    tokens,
    css`
      :host {
        display: block;
        position: relative;
        width: var(--card-width);
      }
      .card {
        box-sizing: border-box;
        padding: 10px 14px 11px;
        border-radius: var(--radius);
        background: var(--info-bg);
        border: 1px solid var(--info-border);
        box-shadow: 0 6px 18px rgba(0, 0, 0, 0.28);
        cursor: default;
        transition: filter 120ms ease-out;
      }
      .card.attention {
        background: var(--attention-bg);
        border-color: var(--attention-border);
      }
      :host(:not([native-hover])) .card:hover {
        filter: brightness(1.08);
      }
      .top {
        display: flex;
        justify-content: space-between;
        gap: 8px;
        color: var(--muted);
        font-size: 11.5px;
      }
      .location {
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
      }
      .title {
        font-weight: 600;
        margin-top: 2px;
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
      }
      .body {
        margin-top: 2px;
        display: -webkit-box;
        -webkit-line-clamp: 2;
        -webkit-box-orient: vertical;
        overflow: hidden;
        overflow-wrap: anywhere;
      }
      .note {
        margin-top: 4px;
        font-style: italic;
        color: var(--muted);
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
      }
      button.close {
        position: absolute;
        top: -6px;
        left: -6px;
        width: 18px;
        height: 18px;
        padding: 0;
        border-radius: 50%;
        border: 1px solid var(--info-border);
        background: var(--close-bg);
        color: var(--text);
        font-size: 12px;
        line-height: 16px;
        opacity: 0;
        transition: opacity 120ms ease-out;
      }
      :host([close-visible]) button.close,
      :host(:not([native-hover]):hover) button.close,
      button.close:focus-visible {
        opacity: 1;
      }
      @media (prefers-reduced-motion: reduce) {
        .card,
        button.close {
          transition: none;
        }
      }
    `,
  ];

  declare card: CardView | undefined;
  declare now: number;
  declare closeVisible: boolean;
  declare nativeHover: boolean;
  declare onActivate: (cardId: CardId) => void;
  declare onClose: (cardId: CardId) => void;

  constructor() {
    super();
    this.card = undefined;
    this.now = 0;
    this.closeVisible = false;
    this.nativeHover = false;
    this.onActivate = () => {};
    this.onClose = () => {};
  }

  override render() {
    const card = this.card;
    if (card === undefined) return nothing;
    const activate = () => this.onActivate(card.card_id);
    const close = (event: Event) => {
      event.stopPropagation();
      this.onClose(card.card_id);
    };
    return html`
      <article class="card ${card.kind}" aria-label=${card.location} @click=${activate}>
        <div class="top">
          <span class="location">${card.location}</span>
          <span class="time">${relativeTime(card.created_ms, this.now)}</span>
        </div>
        ${card.title === null ? nothing : html`<div class="title">${card.title}</div>`}
        ${card.body === null ? nothing : html`<div class="body">${card.body}</div>`}
        ${card.note === null ? nothing : html`<div class="note">${card.note}</div>`}
      </article>
      <button class="close" aria-label="Dismiss" @click=${close}>×</button>
    `;
  }
}

customElements.define("toast-card", ToastCard);
