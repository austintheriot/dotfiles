import type { CardId } from "./bindings/CardId";
import type { PanelViewModel } from "./bindings/PanelViewModel";
import type { UiBootstrap } from "./bindings/UiBootstrap";
import type { UiMessage } from "./bindings/UiMessage";

export const LEAVE_MS = 200;
export const TICK_MS = 30_000;

export type Model = {
  readonly view: PanelViewModel | null;
  readonly nativeHover: boolean;
  readonly hovered: boolean;
  readonly now: number;
  readonly reportedHeight: number | null;
};

export type Msg =
  | { readonly type: "booted"; readonly bootstrap: UiBootstrap }
  | { readonly type: "view_model"; readonly view: PanelViewModel }
  | { readonly type: "native_hover"; readonly hovered: boolean }
  | { readonly type: "pointer"; readonly inside: boolean }
  | { readonly type: "left" }
  | { readonly type: "clicked"; readonly cardId: CardId }
  | { readonly type: "closed"; readonly cardId: CardId }
  | { readonly type: "tick"; readonly now: number }
  | { readonly type: "layout"; readonly heightPx: number };

export type Cmd =
  | { readonly type: "send"; readonly message: UiMessage }
  | { readonly type: "schedule_leave"; readonly delayMs: number }
  | { readonly type: "cancel_leave" };

export function initialModel(now: number): Model {
  return { view: null, nativeHover: false, hovered: false, now, reportedHeight: null };
}

export function closeVisible(model: Model): boolean {
  return model.nativeHover && model.hovered;
}

function newer(current: PanelViewModel | null, incoming: PanelViewModel): PanelViewModel {
  return current !== null && incoming.revision < current.revision ? current : incoming;
}

const send = (message: UiMessage): Cmd => ({ type: "send", message });

export function update(model: Model, msg: Msg): readonly [Model, readonly Cmd[]] {
  switch (msg.type) {
    case "booted":
      return [{ ...model, view: newer(model.view, msg.bootstrap.view_model), nativeHover: msg.bootstrap.native_hover }, []];
    case "view_model":
      return [{ ...model, view: newer(model.view, msg.view) }, []];
    case "native_hover":
      return model.nativeHover ? [{ ...model, hovered: msg.hovered }, []] : [model, []];
    case "pointer":
      if (model.nativeHover) return [model, []];
      if (!msg.inside) return [model, [{ type: "schedule_leave", delayMs: LEAVE_MS }]];
      return model.hovered
        ? [model, [{ type: "cancel_leave" }]]
        : [{ ...model, hovered: true }, [{ type: "cancel_leave" }, send({ type: "hover", hovered: true })]];
    case "left":
      return model.hovered && !model.nativeHover ? [{ ...model, hovered: false }, [send({ type: "hover", hovered: false })]] : [model, []];
    case "clicked":
      return [model, [send({ type: "clicked", card_id: msg.cardId })]];
    case "closed":
      return [model, [send({ type: "closed", card_id: msg.cardId })]];
    case "tick":
      return [{ ...model, now: msg.now }, []];
    case "layout": {
      if (!Number.isFinite(msg.heightPx)) return [model, []];
      const height = Math.max(0, Math.round(msg.heightPx));
      return height === model.reportedHeight
        ? [model, []]
        : [{ ...model, reportedHeight: height }, [send({ type: "layout", content_height_px: height })]];
    }
    default: {
      const unreachable: never = msg;
      return unreachable;
    }
  }
}
