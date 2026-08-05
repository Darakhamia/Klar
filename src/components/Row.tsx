/**
 * The settings row, and the controls that sit in one.
 *
 * The design's rule for this window: rows sit on a single left edge, 2px rules
 * separate them, and red marks only what is currently on. Nothing moves —
 * settings are a place you visit twice a year.
 */

import type { ReactNode } from "react";

export function Row({
  label,
  hint,
  /** Red, for a hint that is telling the user something went wrong. Red marks
   * only what is on or what needs attention — never decoration. */
  alert = false,
  children,
}: {
  label: string;
  hint?: string;
  alert?: boolean;
  children: ReactNode;
}) {
  return (
    <div className="row">
      <div className="row__text">
        <div className="row__label">{label}</div>
        {hint && <div className={alert ? "row__hint row__hint--alert" : "row__hint"}>{hint}</div>}
      </div>
      <div className="row__control">{children}</div>
    </div>
  );
}

/** A choice of two or more, laid out flush. Red marks the one that is on. */
export function Segmented<T extends string>({
  value,
  options,
  onChange,
}: {
  value: T;
  options: { value: T; label: string }[];
  onChange: (value: T) => void;
}) {
  return (
    <div className="seg" role="group">
      {options.map((option) => (
        <button
          key={option.value}
          type="button"
          className="seg__option"
          aria-pressed={option.value === value}
          onClick={() => {
            onChange(option.value);
          }}
        >
          {option.label}
        </button>
      ))}
    </div>
  );
}

export function Select<T extends string | null>({
  value,
  options,
  onChange,
}: {
  value: T;
  options: { value: T; label: string }[];
  onChange: (value: T) => void;
}) {
  return (
    <select
      className="select"
      value={value ?? ""}
      onChange={(event) => {
        const next = event.target.value;
        onChange((next === "" ? null : next) as T);
      }}
    >
      {options.map((option) => (
        <option key={option.label} value={option.value ?? ""}>
          {option.label}
        </option>
      ))}
    </select>
  );
}

/** A machine value — a hotkey, a duration, a size. Archivo with tabular
 * figures and wider tracking, per the type scale. */
export function Figure({ children }: { children: ReactNode }) {
  return <span className="figure">{children}</span>;
}
