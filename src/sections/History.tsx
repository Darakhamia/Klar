/**
 * Everything dictated on this machine.
 *
 * Two things this screen owes the user beyond the list. First, a way to get a
 * sentence back — dictation goes into whatever had focus, and sometimes that
 * was the wrong window. Second, a way to delete, individually and entirely,
 * with no undo and no copy left behind.
 */

import { useCallback, useEffect, useState } from "react";
import { clearHistory, forgetDictation, history, type Dictation } from "../lib/settings";

const LIMIT = 200;

export function History() {
  const [rows, setRows] = useState<Dictation[] | null>(null);
  const [failed, setFailed] = useState<string | null>(null);
  const [confirming, setConfirming] = useState(false);

  const reload = useCallback(() => {
    history(LIMIT)
      .then((loaded) => {
        setRows(loaded);
        setFailed(null);
      })
      .catch((cause: unknown) => {
        setFailed(cause instanceof Error ? cause.message : String(cause));
      });
  }, []);

  useEffect(reload, [reload]);

  if (failed !== null) {
    return <p className="panel__error">{failed}</p>;
  }

  if (rows === null) {
    return <p className="dict__empty">Reading…</p>;
  }

  if (rows.length === 0) {
    return (
      <p className="dict__empty">
        Nothing yet. Every dictation is kept here, on this machine only — nothing is uploaded.
      </p>
    );
  }

  return (
    <>
      <div className="hist__head">
        <span className="figure">
          {rows.length === LIMIT ? `Last ${String(LIMIT)}` : `${String(rows.length)} dictations`}
        </span>
        {confirming ? (
          <span className="hist__confirm">
            <span className="dict__hint">Delete every dictation? The totals in Stats stay.</span>
            <button
              type="button"
              className="btn"
              onClick={() => {
                setConfirming(false);
                clearHistory()
                  .then(reload)
                  .catch((cause: unknown) => {
                    setFailed(cause instanceof Error ? cause.message : String(cause));
                  });
              }}
            >
              Delete
            </button>
            <button
              type="button"
              className="btn btn--ghost"
              onClick={() => {
                setConfirming(false);
              }}
            >
              Keep
            </button>
          </span>
        ) : (
          <button
            type="button"
            className="btn btn--ghost"
            onClick={() => {
              setConfirming(true);
            }}
          >
            Clear history
          </button>
        )}
      </div>

      <ul className="hist">
        {rows.map((row) => (
          <Row key={row.id} row={row} onChanged={reload} />
        ))}
      </ul>
    </>
  );
}

function Row({ row, onChanged }: { row: Dictation; onChanged: () => void }) {
  const [copied, setCopied] = useState(false);

  return (
    <li className="hist__row">
      <div className="hist__meta figure">
        <span>{when(row.at)}</span>
        {row.target !== null && <span>{row.target}</span>}
        <span>{Math.round(row.latencyMs)} ms</span>
        {row.polished && <span>polished</span>}
      </div>

      <p className="hist__text">{row.text}</p>

      {row.raw !== row.text && <p className="hist__raw">heard: {row.raw}</p>}

      <div className="hist__actions">
        <button
          type="button"
          className="btn btn--ghost"
          onClick={() => {
            navigator.clipboard
              .writeText(row.text)
              .then(() => {
                setCopied(true);
                setTimeout(() => {
                  setCopied(false);
                }, 1200);
              })
              .catch(() => {
                setCopied(false);
              });
          }}
        >
          {copied ? "Copied" : "Copy"}
        </button>
        <button
          type="button"
          className="btn btn--ghost"
          onClick={() => {
            forgetDictation(row.id)
              .then(onChanged)
              .catch(() => {
                onChanged();
              });
          }}
        >
          Delete
        </button>
      </div>
    </li>
  );
}

/** Today and yesterday by name; anything older by date. A list of one's own
 * sentences is read by when, and "14:32" is more use than a full timestamp. */
function when(at: number): string {
  const date = new Date(at * 1000);
  const time = date.toLocaleTimeString(undefined, { hour: "2-digit", minute: "2-digit" });

  const midnight = new Date();
  midnight.setHours(0, 0, 0, 0);
  const day = 24 * 60 * 60 * 1000;

  if (date.getTime() >= midnight.getTime()) return time;
  if (date.getTime() >= midnight.getTime() - day) return `Yesterday ${time}`;
  return `${date.toLocaleDateString(undefined, { day: "numeric", month: "short" })} ${time}`;
}
