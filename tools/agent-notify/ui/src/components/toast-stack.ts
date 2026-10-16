import { LitElement, css, html, nothing } from "lit";
import type { CardId } from "../bindings/CardId";
import { closeVisible, type Model, type Msg } from "../model";
import "./finished-row";
import "./toast-card";

export class ToastStack extends LitElement {
  static properties = {
    model: { attribute: false },
    dispatch: { attribute: false },
  };

  static styles = css`
    :host {
      display: flex;
      flex-direction: column;
      gap: 8px;
      padding: 8px 12px 12px;
      background: transparent;
    }
  `;

  declare model: Model | null;
  declare dispatch: (message: Msg) => void;

  constructor() {
    super();
    this.model = null;
    this.dispatch = () => {};
    this.addEventListener("pointerenter", () => this.dispatch({ type: "pointer", inside: true }));
    this.addEventListener("pointerleave", () => this.dispatch({ type: "pointer", inside: false }));
  }

  override render() {
    const model = this.model;
    const view = model?.view;
    if (model === null || view === null || view === undefined) return nothing;
    const activate = (cardId: CardId) => this.dispatch({ type: "clicked", cardId });
    const close = (cardId: CardId) => this.dispatch({ type: "closed", cardId });
    return html`
      ${view.cards.map(
        (card) => html`<toast-card
          .card=${card}
          .now=${model.now}
          .onActivate=${activate}
          .onClose=${close}
          ?close-visible=${closeVisible(model)}
          ?native-hover=${model.nativeHover}
        ></toast-card>`,
      )}
      ${view.finished_row.length === 0 ? nothing : html`<finished-row .entries=${view.finished_row} .onActivate=${activate}></finished-row>`}
    `;
  }

  override updated() {
    this.dispatch({ type: "layout", heightPx: this.getBoundingClientRect().height });
  }
}

customElements.define("toast-stack", ToastStack);
