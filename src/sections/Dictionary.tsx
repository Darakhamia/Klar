/**
 * Teaching Klar a word.
 *
 * The screen has to answer one question the user will have immediately: did it
 * work? So the editor carries a live preview. Type a term, type what you keep
 * hearing instead, and the sample line below shows the substitution running —
 * before ever holding the hotkey and hoping.
 */

import { useCallback, useEffect, useState } from "react";
import {
  dictionary,
  forgetTerm,
  setTermEnabled,
  teach,
  tryDictionary,
  type Entry,
} from "../lib/settings";

export function Dictionary() {
  const [entries, setEntries] = useState<Entry[] | null>(null);
  const [failed, setFailed] = useState<string | null>(null);

  const reload = useCallback(() => {
    dictionary()
      .then((loaded) => {
        setEntries(loaded);
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

  return (
    <>
      <Add onAdded={reload} />

      {entries === null && <p className="dict__empty">Reading the dictionary…</p>}

      {entries !== null && entries.length === 0 && (
        <p className="dict__empty">
          Nothing taught yet. Names, products and jargon are what whisper guesses at — and it
          guesses the same way every time, which is what makes this work.
        </p>
      )}

      {entries !== null && entries.length > 0 && (
        <ul className="dict">
          {entries.map((entry) => (
            <Term key={entry.id} entry={entry} onChanged={reload} />
          ))}
        </ul>
      )}
    </>
  );
}

/** The editor, with the preview that answers "did it work". */
function Add({ onAdded }: { onAdded: () => void }) {
  const [term, setTerm] = useState("");
  const [heard, setHeard] = useState("");
  const [sample, setSample] = useState("");
  const [preview, setPreview] = useState<string | null>(null);
  const [failed, setFailed] = useState<string | null>(null);

  // The preview runs against what is saved, so it shows the rules working
  // rather than guessing at them in TypeScript. Debounced: this is a database
  // round trip on every keystroke otherwise.
  useEffect(() => {
    // An empty box shows nothing, and that is decided at render rather than by
    // clearing state here — setting state straight from an effect body is a
    // cascading render, and the condition is already known below.
    if (!sample.trim()) return;

    const timer = setTimeout(() => {
      tryDictionary(sample)
        .then(setPreview)
        .catch(() => {
          setPreview(null);
        });
    }, 200);
    return () => {
      clearTimeout(timer);
    };
  }, [sample]);

  const save = () => {
    const replacements = heard
      .split(",")
      .map((one) => one.trim())
      .filter((one) => one.length > 0);

    if (!term.trim() || replacements.length === 0) {
      setFailed("A term needs both a spelling and at least one thing it sounds like.");
      return;
    }

    teach(term, replacements)
      .then(() => {
        setTerm("");
        setHeard("");
        setFailed(null);
        onAdded();
        // Re-run the preview against the entry that was just saved.
        if (sample.trim()) {
          tryDictionary(sample)
            .then(setPreview)
            .catch(() => {
              setPreview(null);
            });
        }
      })
      .catch((cause: unknown) => {
        setFailed(cause instanceof Error ? cause.message : String(cause));
      });
  };

  return (
    <div className="dict__add">
      <div className="dict__fields">
        <label className="dict__field">
          <span className="label">Spelling you want</span>
          <input
            className="input"
            value={term}
            placeholder="Anthropic"
            onChange={(event) => {
              setTerm(event.target.value);
            }}
          />
        </label>

        <label className="dict__field">
          <span className="label">What you hear instead</span>
          <input
            className="input"
            value={heard}
            placeholder="anthropik, and thropic"
            onChange={(event) => {
              setHeard(event.target.value);
            }}
            onKeyDown={(event) => {
              if (event.key === "Enter") save();
            }}
          />
        </label>

        <button type="button" className="btn" onClick={save}>
          Teach
        </button>
      </div>

      <p className="dict__hint">
        Separate several with commas. Matching ignores case and stops at word boundaries, so a term
        for <em>Ana</em> leaves <em>analysis</em> alone.
      </p>

      {failed !== null && <p className="panel__error">{failed}</p>}

      <label className="dict__field dict__try">
        <span className="label">Try a sentence</span>
        <input
          className="input"
          value={sample}
          placeholder="I work at anthropik"
          onChange={(event) => {
            setSample(event.target.value);
          }}
        />
      </label>

      {sample.trim() !== "" && preview !== null && (
        <p className={preview === sample ? "dict__preview" : "dict__preview dict__preview--changed"}>
          {preview === sample ? "Nothing matched." : preview}
        </p>
      )}
    </div>
  );
}

function Term({ entry, onChanged }: { entry: Entry; onChanged: () => void }) {
  const id = entry.id;
  if (id === undefined) return null;

  return (
    <li className={entry.enabled ? "dict__row" : "dict__row dict__row--off"}>
      <div className="dict__word">
        <span className="dict__term">{entry.term}</span>
        <span className="dict__heard">{entry.replacements.join(", ")}</span>
      </div>
      <div className="dict__actions">
        <button
          type="button"
          className="btn btn--ghost"
          onClick={() => {
            setTermEnabled(id, !entry.enabled)
              .then(onChanged)
              .catch(() => {
                onChanged();
              });
          }}
        >
          {entry.enabled ? "Off" : "On"}
        </button>
        <button
          type="button"
          className="btn btn--ghost"
          onClick={() => {
            forgetTerm(id)
              .then(onChanged)
              .catch(() => {
                onChanged();
              });
          }}
        >
          Forget
        </button>
      </div>
    </li>
  );
}
