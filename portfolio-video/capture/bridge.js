/**
 * The Tauri shell, as far as the frontend can tell.
 *
 * Injected before any page script runs (Playwright's addInitScript). It
 * provides the two objects `@tauri-apps/api` reaches for —
 * `__TAURI_INTERNALS__` and `__TAURI_EVENT_PLUGIN_INTERNALS__` — and answers
 * every command with the demo data in `window.__KLAR_DEMO__`. The app's own
 * code is untouched: `isTauri()` is true, `invoke` resolves, `listen`
 * subscribes, and events are delivered the way the Rust side delivers them.
 *
 * `window.__klar` is for the capture script: emit an engine event, and hold
 * `performance.now()` on a virtual clock so time-based UI (the overlay's
 * elapsed counter) reads what the shot needs rather than the wall clock.
 */
(() => {
  const demo = window.__KLAR_DEMO__;
  const applyDictionary = window.__KLAR_APPLY_DICTIONARY__;

  const callbacks = new Map();
  const listeners = new Map();
  let nextId = 1;

  const commands = {
    app_version: () => demo.version,
    settings_get: () => demo.settings,
    settings_set: ({ settings }) => {
      demo.settings = settings;
    },
    models: () => demo.models,
    audio_devices: () => demo.devices,
    acceleration: () => demo.acceleration,
    polish_status: () => demo.polish,
    dictionary: () => demo.dictionary,
    dictionary_try: ({ text }) => applyDictionary(demo.dictionary, text),
    history: ({ limit }) => demo.history.slice(0, limit),
    stats: () => demo.stats,
    default_hotkey: () => demo.settings.hotkey,
    permission_states: () => [
      { permission: "microphone", state: "granted" },
      { permission: "accessibility", state: "notApplicable" },
    ],
    update_check: () => null,
    ui_log: () => undefined,
    hotkey_suspend: () => undefined,
  };

  window.isTauri = true;

  window.__TAURI_INTERNALS__ = {
    metadata: {
      currentWindow: { label: demo.window },
      currentWebview: { windowLabel: demo.window, label: demo.window },
    },
    transformCallback(callback, once) {
      const id = nextId++;
      callbacks.set(id, (payload) => {
        if (once) callbacks.delete(id);
        return callback(payload);
      });
      return id;
    },
    unregisterCallback(id) {
      callbacks.delete(id);
    },
    convertFileSrc: (path) => path,
    async invoke(cmd, args = {}) {
      if (cmd === "plugin:event|listen") {
        const ids = listeners.get(args.event) ?? [];
        ids.push(args.handler);
        listeners.set(args.event, ids);
        return args.handler;
      }
      if (cmd === "plugin:event|unlisten") return undefined;
      const handler = commands[cmd];
      if (!handler) throw new Error(`demo shell: no answer for "${cmd}"`);
      return structuredClone(handler(args));
    },
  };

  window.__TAURI_EVENT_PLUGIN_INTERNALS__ = {
    unregisterListener(event, id) {
      const ids = listeners.get(event) ?? [];
      listeners.set(
        event,
        ids.filter((one) => one !== id),
      );
    },
  };

  const realNow = performance.now.bind(performance);
  let virtual = null;
  performance.now = () => (virtual === null ? realNow() : virtual);

  window.__klar = {
    emit(event, payload) {
      for (const id of listeners.get(event) ?? []) callbacks.get(id)?.({ event, id, payload });
    },
    /** Pin `performance.now()` to `ms`, or release it with null. */
    setTime(ms) {
      virtual = ms;
    },
  };
})();
