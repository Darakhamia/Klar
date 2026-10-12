import { useEffect, useLayoutEffect, useRef, useState, type CSSProperties } from "react";
import { continueRender, delayRender, useCurrentFrame } from "remotion";
import { GPU } from "../demo";
import type { Layout } from "../format";
import { aim, ramp, resolve } from "../motion";
import { T } from "../timeline";
import { Camera } from "../ui/Camera";
import { SettingsWindow } from "../ui/SettingsWindow";

/** The sentence the Voice scene is about. */
const PRIVATE = "Speech is recognised on this machine and never leaves it.";

/**
 * Whether the real components have drawn their data yet. History and Voice
 * fetch on mount and draw a placeholder until the answer comes back, and a
 * frame captured in between would show "Reading…".
 */
function settled(root: HTMLElement): boolean {
  const text = root.textContent;
  return (
    root.querySelector(".hist__row") !== null &&
    text.includes(GPU) &&
    !text.includes("Asking the local model server")
  );
}

const HIGHLIGHT = "kv-sentence";

/** Lays the highlight over the first `share` of the sentence, or clears it. */
function paint(voice: HTMLElement, share: number): void {
  const text = voice.querySelector(".panel__body > .row .row__hint")?.firstChild;
  if (share <= 0 || !(text instanceof Text) || !text.data.startsWith(PRIVATE)) {
    CSS.highlights.delete(HIGHLIGHT);
    return;
  }
  const range = new Range();
  range.setStart(text, 0);
  range.setEnd(text, Math.round(PRIVATE.length * share));
  CSS.highlights.set(HIGHLIGHT, new Highlight(range));
}

/** Scenes 4 and 5: History, then Settings → Voice, in the dark theme. */
export function SettingsScene({ layout }: { layout: Layout }) {
  const frame = useCurrentFrame();
  const root = useRef<HTMLDivElement>(null);
  const voice = useRef<HTMLDivElement>(null);

  // Taken while rendering, not in an effect, so no frame can be captured
  // before the hold is in place.
  const [hold] = useState(() => delayRender("History and Voice drawing their data"));

  useEffect(() => {
    let request = 0;
    const check = () => {
      if (root.current !== null && settled(root.current)) {
        continueRender(hold);
      } else {
        request = requestAnimationFrame(check);
      }
    };
    check();
    return () => {
      cancelAnimationFrame(request);
      continueRender(hold);
    };
  }, [hold]);

  // Marks the sentence character by character with the Custom Highlight API,
  // which paints over the real text node without touching Voice's markup.
  // React rewrites that node's data when Voice re-renders, which collapses any
  // range on it, so the mark is laid again after every change underneath.
  const mark = ramp(frame, T.sweep, T.sweepFrames, resolve);
  const marked = useRef(0);
  useLayoutEffect(() => {
    marked.current = mark;
    if (voice.current !== null) paint(voice.current, mark);
  });
  useEffect(() => {
    const node = voice.current;
    if (node === null) return;
    const observer = new MutationObserver(() => {
      paint(node, marked.current);
    });
    observer.observe(node, { subtree: true, childList: true, characterData: true });
    return () => {
      observer.disconnect();
      CSS.highlights.delete(HIGHLIGHT);
    };
  }, []);

  const settle = ramp(frame, T.toDark, 24, resolve);
  const leave = ramp(frame, T.toEnd, 12, resolve);
  const dim = ramp(frame, T.voiceFocus, 24, resolve);
  const chrome = ramp(frame, T.voiceFocus, 12, resolve);
  const list = ramp(frame, T.listFocus, 24, resolve) * (1 - ramp(frame, T.listRelease, 16, resolve));
  const listChrome = ramp(frame, T.listFocus, 12, resolve) * (1 - ramp(frame, T.listRelease, 16, resolve));

  const focus = {
    "--kv-dim": String(1 - 0.86 * dim),
    "--kv-dim-chrome": String(1 - 0.8 * chrome),
    position: "absolute",
    inset: 0,
    opacity: frame >= T.toVoice ? 1 : 0,
  } as CSSProperties;

  return (
    <div className="kv-layer" style={{ background: "var(--kv-ink)" }}>
      <Camera layout={layout} shot={aim(frame, layout.settingsShots)}>
        <div
          ref={root}
          style={{
            position: "absolute",
            left: 0,
            top: 0,
            width: 900,
            height: 652,
            opacity: 1 - leave,
            transform: `scale(${String(1.02 - 0.02 * settle - 0.02 * leave)})`,
          }}
        >
          <div className="kv-focus-list" style={
              {
                "--kv-list-dim": String(1 - 0.78 * list),
                "--kv-list-chrome": String(1 - 0.8 * listChrome),
              } as CSSProperties
            }>
            <SettingsWindow section="History" />
          </div>
          <div ref={voice} className="kv-focus" style={focus}>
            <SettingsWindow section="Voice" />
          </div>
        </div>
      </Camera>
    </div>
  );
}
