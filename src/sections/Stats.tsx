/**
 * How much this has actually been used.
 *
 * The one number worth putting first is time saved, and it is only honest if
 * it is a comparison the user can check: how long these words would have taken
 * to type, minus how long they took to say. The typing speed is theirs to set,
 * because a number computed against a speed nobody agreed to is decoration.
 */

import { useCallback, useEffect, useState } from "react";
import { Row } from "../components/Row";
import { clearStats, stats, type StatsReport } from "../lib/settings";

const DAYS = 14;

/** What an average person types. Adjustable, because the claim depends on it. */
const DEFAULT_WPM = 40;

export function Stats() {
  const [report, setReport] = useState<StatsReport | null>(null);
  const [failed, setFailed] = useState<string | null>(null);
  const [wpm, setWpm] = useState(DEFAULT_WPM);
  const [confirming, setConfirming] = useState(false);

  const reload = useCallback(() => {
    stats(DAYS)
      .then((loaded) => {
        setReport(loaded);
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

  if (report === null) {
    return <p className="dict__empty">Reading…</p>;
  }

  if (report.totals.dictations === 0) {
    return (
      <p className="dict__empty">
        Nothing yet. Counts are kept per day and survive clearing the history — the text is the
        private part, the totals are not.
      </p>
    );
  }

  const { totals } = report;
  const typingSeconds = (totals.words / Math.max(wpm, 1)) * 60;
  const spokenSeconds = totals.audioMs / 1000;
  const saved = typingSeconds - spokenSeconds;

  return (
    <>
      <div className="stat__row">
        <Figure value={totals.dictations.toLocaleString()} label="dictations" />
        <Figure value={totals.words.toLocaleString()} label="words" />
        <Figure value={totals.days.toLocaleString()} label="days used" />
        <Figure value={duration(spokenSeconds)} label="spoken" />
      </div>

      <Row
        label="Time saved"
        hint={`Against typing these ${totals.words.toLocaleString()} words yourself, minus the ${duration(
          spokenSeconds,
        )} you spent saying them.`}
      >
        <span className="figure">{saved > 0 ? duration(saved) : "—"}</span>
      </Row>

      <Row label="Your typing speed" hint="The number above is only as honest as this one.">
        <input
          className="input input--narrow"
          type="number"
          min={5}
          max={200}
          value={wpm}
          onChange={(event) => {
            setWpm(Number(event.target.value) || DEFAULT_WPM);
          }}
        />
      </Row>

      <ul className="days">
        {report.daily.map((day) => (
          <li key={day.day} className="days__row">
            <span className="figure">{day.day}</span>
            <span className="days__bar" style={{ width: bar(day.words, report.daily) }} />
            <span className="days__count">
              {day.words.toLocaleString()} words · {day.dictations}
            </span>
          </li>
        ))}
      </ul>

      <Row
        label="Erase the totals"
        hint="Separate from clearing the history: one forgets what you said, the other forgets that you were here."
      >
        {confirming ? (
          <span className="hist__confirm">
            <button
              type="button"
              className="btn"
              onClick={() => {
                setConfirming(false);
                clearStats()
                  .then(reload)
                  .catch((cause: unknown) => {
                    setFailed(cause instanceof Error ? cause.message : String(cause));
                  });
              }}
            >
              Erase
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
            Erase
          </button>
        )}
      </Row>
    </>
  );
}

function Figure({ value, label }: { value: string; label: string }) {
  return (
    <div className="stat">
      <div className="stat__value">{value}</div>
      <div className="stat__label label">{label}</div>
    </div>
  );
}

/** Relative to the busiest day on screen, so the shape of a fortnight reads at
 * a glance without an axis. */
function bar(words: number, days: { words: number }[]): string {
  const most = Math.max(...days.map((day) => day.words), 1);
  return `${String(Math.max(2, Math.round((words / most) * 100)))}%`;
}

function duration(seconds: number): string {
  const whole = Math.round(seconds);
  if (whole >= 3600) return `${String(Math.floor(whole / 3600))} h ${String(Math.floor((whole % 3600) / 60))} m`;
  if (whole >= 60) return `${String(Math.floor(whole / 60))} m`;
  return `${String(whole)} s`;
}
