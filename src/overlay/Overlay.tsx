/**
 * The dictation overlay — direction A, "Collapse", from the design.
 *
 * The waveform *is* the text: the bars flatten, widen into word blocks, and the
 * sentence lands in their place. It is the one place the product's single idea
 * happens in front of the user, so it gets the attention.
 *
 * Rules the design sets, which the code below keeps:
 *
 * - **Red means live.** The accent appears only while audio is being captured.
 *   Once there is text the overlay goes grey and leaves.
 * - **Width earns itself.** 260 px at rest, growing only when there are real
 *   words to show, and never past 360.
 * - **Error waits.** Every other state dismisses itself.
 */

import type { Engine, PipelineState } from "./useEngine";
import "./overlay.css";

/** What the user is told, per state. */
const STATUS: Record<PipelineState, string> = {
  idle: "",
  recording: "Listening",
  transcribing: "Thinking",
  polishing: "Polishing",
  injecting: "Done",
  error: "Error",
};

export function Overlay({ engine }: { engine: Engine }) {
  const { state, levels, text, error, elapsed } = engine;

  if (state === "idle") return null;

  const live = state === "recording";
  const showText = text.length > 0 && !live;

  return (
    <div
      className="pill"
      data-state={state}
      data-wide={showText || state === "error" ? "true" : "false"}
    >
      <Waveform levels={levels} live={live} collapsed={showText} />

      <div className="pill__body">
        <div className="pill__status">{STATUS[state]}</div>
        <div className="pill__meta">
          {state === "error"
            ? (error ?? "Something went wrong")
            : showText
              ? text
              : `${formatElapsed(elapsed)} — hold ${hotkeyLabel()}`}
        </div>
      </div>
    </div>
  );
}

/**
 * Twelve bars while listening; the same twelve flattened into word blocks once
 * the audio has become text.
 */
function Waveform({
  levels,
  live,
  collapsed,
}: {
  levels: number[];
  live: boolean;
  collapsed: boolean;
}) {
  return (
    <div className="wave" data-collapsed={collapsed ? "true" : "false"}>
      {levels.map((level, index) => (
        <span
          key={index}
          className="wave__bar"
          style={{
            // A square-root curve so ordinary speech fills most of the range;
            // linear peak barely moves for anything but a shout.
            height: collapsed ? "3px" : `${Math.max(3, Math.sqrt(level) * 30)}px`,
            opacity: live ? 1 : 0.45,
          }}
        />
      ))}
    </div>
  );
}

function formatElapsed(seconds: number): string {
  const whole = Math.floor(seconds);
  return `${Math.floor(whole / 60)}:${String(whole % 60).padStart(2, "0")}`;
}

/**
 * The binding, as the design writes it. Comes from settings once that window
 * exists; until then it is the platform default the engine registers.
 */
function hotkeyLabel(): string {
  return "CTRL SPACE";
}
