/**
 * The last step — hotkey, model, first sentence.
 *
 * The hotkey is settled first so the wait has something to look at, and the
 * test field is visibly disabled rather than hidden while the model downloads.
 * Nothing here fakes progress: every number comes from the download task, and
 * the field only opens once the engine has said it is ready.
 */

import { useCallback, useEffect, useRef, useState } from "react";
import {
  MODEL_EVENT,
  downloadModel,
  formatBinding,
  restartEngine,
  subscribe,
  type Binding,
  type ModelEvent,
} from "../lib/ipc";
import { listModels, type ModelStatus, type Settings } from "../lib/settings";
import type { EngineView } from "./useEngine";

/** Voice activity detection. Under a megabyte, and required — the streaming
 * pass does not run without it — so it is fetched alongside the speech model
 * and only ever surfaces if it fails. */
const VAD = "silero-vad";

const MB = 1024 * 1024;
const megabytes = (bytes: number) => String(Math.round(bytes / MB));

/** The first progress sample of a download, which is what makes the estimate
 * below a measurement rather than a guess. */
interface Anchor {
  at: number;
  received: number;
}

export function Setup({
  settings,
  os,
  engine,
  onDone,
}: {
  settings: Settings;
  os: string;
  engine: EngineView;
  onDone: () => void;
}) {
  const [models, setModels] = useState<ModelStatus[] | null>(null);
  const [progress, setProgress] = useState<Record<string, ModelEvent>>({});
  const [error, setError] = useState<string | null>(null);

  const [anchors, setAnchors] = useState<Record<string, Anchor>>({});
  const asked = useRef(false);
  const restarted = useRef(false);

  useEffect(() => {
    return subscribe<ModelEvent>(
      MODEL_EVENT,
      (payload) => {
        if (payload.phase === "downloading") {
          setAnchors((previous) =>
            payload.id in previous
              ? previous
              : {
                  ...previous,
                  [payload.id]: { at: Date.now(), received: payload.received },
                },
          );
        }
        if (payload.phase === "failed") setError(`${payload.id}: ${payload.message}`);
        setProgress((previous) => ({ ...previous, [payload.id]: payload }));
      },
      setError,
    );
  }, []);

  useEffect(() => {
    let cancelled = false;
    listModels()
      .then((catalogue) => {
        if (!cancelled) setModels(catalogue);
      })
      .catch((cause: unknown) => {
        if (!cancelled) setError(cause instanceof Error ? cause.message : String(cause));
      });
    return () => {
      cancelled = true;
    };
  }, []);

  const fetchMissing = useCallback((catalogue: ModelStatus[], speechModel: string) => {
    for (const id of [VAD, speechModel]) {
      if (catalogue.find((model) => model.id === id)?.installed) continue;
      downloadModel(id).catch((cause: unknown) => {
        setError(cause instanceof Error ? cause.message : String(cause));
      });
    }
  }, []);

  // Start as soon as we know what is missing. A ref rather than state: this
  // runs once, not once per render.
  useEffect(() => {
    if (models === null || asked.current) return;
    asked.current = true;
    fetchMissing(models, settings.model);
  }, [models, settings.model, fetchMissing]);

  const present = (id: string) =>
    progress[id]?.phase === "done" || (models?.find((m) => m.id === id)?.installed ?? false);

  const speech = models?.find((model) => model.id === settings.model) ?? null;
  const ready = models !== null && present(settings.model) && present(VAD);

  // The engine gave up at startup if it found nothing to load. Now there is.
  useEffect(() => {
    if (!ready || restarted.current) return;
    restarted.current = true;
    restartEngine().catch((cause: unknown) => {
      setError(cause instanceof Error ? cause.message : String(cause));
    });
  }, [ready]);

  const retry = () => {
    setError(null);
    setProgress({});
    setAnchors({});
    if (models) fetchMissing(models, settings.model);
  };

  return (
    <>
      <h2>{engine.said ? "That was it" : "Pick a key and say something"}</h2>

      <div className="ob-row">
        <div>
          <div className="ob-row__label">Hotkey</div>
          <div className="ob-row__hint">Hold it anywhere to dictate</div>
        </div>
        <div className="ob-key figure">{formatBinding(settings.hotkey, os)}</div>
      </div>

      <ModelRow
        id={settings.model}
        size={speech?.size ?? ""}
        event={progress[settings.model]}
        anchor={anchors[settings.model]}
        installed={present(settings.model)}
      />

      {error && (
        <p className="ob__error">
          {error}{" "}
          <button type="button" className="ob__btn ob__btn--ghost" onClick={retry}>
            Try again
          </button>
        </p>
      )}

      <Try engine={engine} ready={ready} hotkey={settings.hotkey} os={os} />

      <div className="ob__actions ob__actions--end">
        {ready && engine.hotkey && !engine.said && (
          <button type="button" className="ob__btn ob__btn--ghost" onClick={onDone}>
            Finish without testing
          </button>
        )}
        <button
          type="button"
          className="ob__btn ob__btn--primary"
          disabled={!engine.said}
          onClick={onDone}
        >
          Done
        </button>
      </div>
    </>
  );
}

/** One model's progress. Bytes rather than a percentage while it runs — half a
 * gigabyte is a number worth seeing — and it says when it is hashing, because
 * on a slow disk that is not instant and would otherwise look like a hang. */
function ModelRow({
  id,
  size,
  event,
  anchor,
  installed,
}: {
  id: string;
  size: string;
  event: ModelEvent | undefined;
  anchor: Anchor | undefined;
  installed: boolean;
}) {
  if (installed) {
    return (
      <div className="ob-row">
        <div className="ob-row__label">Speech model — {id}</div>
        <div className="label label--on">Ready{size && ` — ${size}`}</div>
      </div>
    );
  }

  const received = event?.phase === "downloading" ? event.received : 0;
  const total = event?.phase === "downloading" ? event.total : 0;
  const fraction = total > 0 ? received / total : 0;
  const left = event?.phase === "downloading" ? remaining(event, anchor) : null;

  return (
    <div className="ob-progress">
      <div className="ob-progress__head">
        <div className="ob-row__label">Speech model — {id}</div>
        <div className="figure">
          {total > 0 ? `${megabytes(received)} / ${megabytes(total)} MB` : size}
        </div>
      </div>
      <div className="ob-progress__track">
        <div className="ob-progress__fill" style={{ width: `${String(fraction * 100)}%` }} />
      </div>
      <div className="label">
        {event?.phase === "verifying"
          ? "Checking the download"
          : event?.phase === "failed"
            ? "Download failed"
            : left
              ? `${left} — downloads once, then runs offline`
              : "Downloads once, then runs offline"}
      </div>
    </div>
  );
}

/** How much longer, from this download's own measured rate. Nothing is shown
 * until there are five seconds of it, which is the point where the number
 * stops swinging. */
function remaining(
  event: Extract<ModelEvent, { phase: "downloading" }>,
  anchor: Anchor | undefined,
): string | null {
  if (!anchor) return null;

  const elapsed = (Date.now() - anchor.at) / 1000;
  const done = event.received - anchor.received;
  if (elapsed < 5 || done <= 0) return null;

  const seconds = (event.total - event.received) / (done / elapsed);
  if (!Number.isFinite(seconds)) return null;

  return seconds < 90
    ? `${String(Math.max(1, Math.round(seconds)))} sec left`
    : `${String(Math.round(seconds / 60))} min left`;
}

/** The test field. It is a real text box and the text really is injected into
 * it — the same clipboard paste every other app gets — so what happens here is
 * the app working, not a preview of it. */
function Try({
  engine,
  ready,
  hotkey,
  os,
}: {
  engine: EngineView;
  ready: boolean;
  hotkey: Binding;
  os: string;
}) {
  if (engine.said) {
    return (
      <div className="ob-try">
        <div className="ob-row__label">You said</div>
        <div className="ob-said">{engine.said}</div>
        <p className="ob__aside">
          Straight from the speech model. The stage that tidies up filler and self-corrections
          arrives in the next milestone.
        </p>
      </div>
    );
  }

  if (!ready) {
    return (
      <div className="ob-try ob-try--waiting">
        <div className="ob-row__label">Try it</div>
        <div className="ob-try__empty">Available when the model finishes</div>
      </div>
    );
  }

  if (!engine.hotkey) {
    return (
      <div className="ob-try ob-try--waiting">
        <div className="ob-row__label">Try it</div>
        <div className="ob-try__empty">{engine.error ?? "Loading the speech model…"}</div>
      </div>
    );
  }

  return (
    <div className="ob-try">
      <div className="ob-row__label">Try it</div>
      <textarea
        className="ob-try__field"
        rows={2}
        placeholder={`Click here, hold ${formatBinding(hotkey, os)} and say something.`}
      />
    </div>
  );
}
