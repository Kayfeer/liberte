import { listen, type EventCallback, type UnlistenFn } from "@tauri-apps/api/event";

/** Dispose late registrations as well as already-active listeners. */
export function createEventSubscriptions(onError: (error: unknown) => void) {
  let disposed = false;
  const unlisteners: UnlistenFn[] = [];
  return {
    add<T>(name: string, callback: EventCallback<T>) {
      void listen<T>(name, (event) => {
        if (!disposed) callback(event);
      }).then((unlisten) => {
        if (disposed) unlisten();
        else unlisteners.push(unlisten);
      }).catch((error: unknown) => {
        if (!disposed) onError(error);
      });
    },
    dispose() {
      disposed = true;
      unlisteners.splice(0).forEach((unlisten) => unlisten());
    },
  };
}
