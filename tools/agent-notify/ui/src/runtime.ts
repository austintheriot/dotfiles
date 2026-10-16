import type { ToastStack } from "./components/toast-stack";
import { TICK_MS, initialModel, update, type Cmd, type Model, type Msg } from "./model";
import type { UiServices, Unsubscribe } from "./services";

export type Runtime = {
  dispatch(msg: Msg): void;
  current(): Model;
  stop(): void;
};

export async function start(root: ToastStack, services: UiServices): Promise<Runtime> {
  let model = initialModel(services.now());
  let cancelLeave: Unsubscribe | null = null;
  let cancelTick: Unsubscribe | null = null;
  let stopped = false;
  const subscriptions: Unsubscribe[] = [];

  const run = (cmd: Cmd): void => {
    switch (cmd.type) {
      case "send":
        services.send(cmd.message);
        return;
      case "schedule_leave":
        cancelLeave?.();
        cancelLeave = services.setTimer(() => {
          cancelLeave = null;
          dispatch({ type: "left" });
        }, cmd.delayMs);
        return;
      case "cancel_leave":
        cancelLeave?.();
        cancelLeave = null;
        return;
      default: {
        const unreachable: never = cmd;
        return unreachable;
      }
    }
  };

  const dispatch = (msg: Msg): void => {
    const [next, cmds] = update(model, msg);
    model = next;
    root.model = model;
    cmds.forEach(run);
  };

  const scheduleTick = (): void => {
    cancelTick?.();
    cancelTick = services.setTimer(tick, TICK_MS);
  };

  const tick = (): void => {
    cancelTick = null;
    if (stopped) return;
    dispatch({ type: "tick", now: services.now() });
    scheduleTick();
  };

  root.dispatch = dispatch;
  root.model = model;
  subscriptions.push(await services.onViewModel((view) => dispatch({ type: "view_model", view })));
  subscriptions.push(await services.onHover((hovered) => dispatch({ type: "native_hover", hovered })));
  dispatch({ type: "booted", bootstrap: await services.bootstrap() });
  scheduleTick();

  return {
    dispatch,
    current: () => model,
    stop: () => {
      stopped = true;
      cancelLeave?.();
      cancelTick?.();
      cancelTick = null;
      subscriptions.forEach((unsubscribe) => unsubscribe());
    },
  };
}
