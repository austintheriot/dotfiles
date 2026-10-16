import type { PanelViewModel } from "./bindings/PanelViewModel";
import type { UiBootstrap } from "./bindings/UiBootstrap";
import type { UiMessage } from "./bindings/UiMessage";

export type Unsubscribe = () => void;

export interface UiServices {
  bootstrap(): Promise<UiBootstrap>;
  send(message: UiMessage): void;
  onViewModel(listener: (view: PanelViewModel) => void): Promise<Unsubscribe>;
  onHover(listener: (hovered: boolean) => void): Promise<Unsubscribe>;
  now(): number;
  setTimer(callback: () => void, delayMs: number): Unsubscribe;
}
