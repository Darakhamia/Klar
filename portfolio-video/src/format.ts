/**
 * The two compositions. Same story, laid out twice: the square is recomposed
 * — its own window sizes and camera paths — rather than cropped from 16:9.
 *
 * Everything is in stage pixels: a logical canvas drawn at 1.5×, so the app's
 * own sizes (a 260 × 64 pill, a 900 × 620 window) stay in true proportion and
 * the camera's zoom does the enlarging.
 *
 * The rule every shot keeps: a subject is either whole, with room around it,
 * or framed tight enough that the crop is plainly a close-up. Nothing
 * important lands on the edge of the frame by accident.
 */

import { at, fit, fitAbove, from, nudge, span, type Framing, type Rect, type Shot } from "./motion";

export type Format = "wide" | "square";

export type Box = Rect;

export interface Layout {
  width: number;
  height: number;
  /** Stage pixels to output pixels. */
  scale: number;
  /** The one left edge captions and the end card share. */
  margin: number;
  captionBottom: number;
  /** The email window, the ghost line and the pill stack as one centred group. */
  mail: Box;
  ghost: { top: number; width: number };
  pillTop: number;
  composeShots: readonly Shot[];
  /** The settings window sits at 0, 0 of its own scene. */
  settingsShots: readonly Shot[];
  wordmark: number;
}

/**
 * Where things are inside the 900 × 652 settings window, measured from the
 * real History and Voice components as rendered here. If their CSS changes,
 * measure again.
 */
const WINDOW: Box = { x: 0, y: 0, width: 900, height: 652 };
/** Bottom of the title bar: the app's own content starts here. */
const CONTENT_TOP = 34;
/** Right edge of the sidebar, its 2 px rule included. */
const SIDEBAR_EDGE = 202;
/** Above the "History" heading, below the title bar. */
const ABOVE_HEADING = 40;
/** Below the 2 px rule under the heading. */
const BELOW_RULE = 108;

/** Where the camera is for each beat of the email scene. */
interface ComposeBeats {
  establish: Framing;
  /** Listening: a slow push onto what is being said and the pill hearing it. */
  listen: Framing;
  /** Done: the whole group again as the sentence lands. This is the poster. */
  done: Framing;
  /** The pill has gone: in on the finished email. */
  email: Framing;
}

function composeShots(beats: ComposeBeats): Shot[] {
  return [
    at(30, beats.establish),
    at(84, nudge(beats.establish)),
    at(180, beats.listen),
    // Polishing: still closer on the pill, never so close that the window's
    // sides reach the frame.
    at(222, nudge(beats.listen, 1.02)),
    at(252, beats.done),
    at(290, nudge(beats.done)),
    at(326, beats.email),
    at(336, nudge(beats.email, 1.005)),
  ];
}

function settingsShots(
  view: { width: number; height: number },
  beats: { window: { fill: number; bottom: number }; entryTop: number; rowTop: number },
): Shot[] {
  const whole = fitAbove(view, WINDOW, beats.window.fill, beats.window.bottom);
  // The list column edge to edge — the newest entry and its "heard:" line,
  // with the sidebar's rule and the window's border just out of frame.
  const entry = span(view, SIDEBAR_EDGE + 10, WINDOW.width - 10, beats.entryTop);
  // The Processing row edge to edge, sidebar and window border out of frame.
  const row = span(view, SIDEBAR_EDGE + 8, WINDOW.width - 10, beats.rowTop);
  return [
    at(330, whole),
    at(350, nudge(whole)),
    at(414, entry),
    at(426, nudge(entry, 1.006)),
    // Back out, so the click from History to Voice in the sidebar is seen.
    at(447, whole),
    at(462, nudge(whole)),
    at(520, row),
    at(566, nudge(row, 1.01)),
  ];
}

const WIDE = { width: 1280, height: 720 };
const SQUARE = { width: 720, height: 720 };

// The email scene: the window, and the group it makes with the ghost line
// and the pill below it (measured from MailWindow, GhostLine and the pill).
const wideMail: Box = { x: 320, y: 150, width: 640, height: 290 };
const wideGroup: Box = { x: 320, y: 150, width: 640, height: 412 };
const squareMail: Box = { x: 60, y: 122, width: 600, height: 330 };
const squareGroup: Box = { x: 60, y: 122, width: 600, height: 475 };

export const LAYOUTS: Record<Format, Layout> = {
  wide: {
    width: WIDE.width,
    height: WIDE.height,
    scale: 1.5,
    margin: 64,
    captionBottom: 44,
    mail: wideMail,
    ghost: { top: 462, width: 640 },
    pillTop: 498,
    composeShots: composeShots({
      establish: fit(WIDE, wideGroup, 0.74),
      // The ghost line fills 78 % of the width. The frame's top edge runs
      // through the empty foot of the email body, below the caret.
      listen: from(WIDE, 640, 330, (0.78 * WIDE.width) / 538),
      done: fitAbove(WIDE, wideGroup, 0.8, 614),
      email: fit(WIDE, wideMail, 0.87),
    }),
    settingsShots: settingsShots(WIDE, {
      window: { fill: 0.82, bottom: 630 },
      entryTop: ABOVE_HEADING,
      rowTop: CONTENT_TOP,
    }),
    wordmark: 150,
  },
  square: {
    width: SQUARE.width,
    height: SQUARE.height,
    scale: 1.5,
    margin: 44,
    captionBottom: 32,
    mail: squareMail,
    ghost: { top: 474, width: 560 },
    pillTop: 533,
    composeShots: composeShots({
      establish: fit(SQUARE, squareGroup, 0.84),
      // The square is narrow enough that the whole group reads at size, so
      // the push stays on all of it: closer would put the window's sides, and
      // its Send button, on the edge of the frame.
      listen: fitAbove(SQUARE, squareGroup, 0.9, 622),
      done: fitAbove(SQUARE, squareGroup, 0.86, 618),
      email: fit(SQUARE, squareMail, 0.9),
    }),
    settingsShots: settingsShots(SQUARE, {
      window: { fill: 0.9, bottom: 560 },
      // Below the heading's rule: the square is too short to give the heading
      // room above it, and the window's bottom edge then clears the caption.
      entryTop: BELOW_RULE,
      // The window's bottom edge sits at 640, above the band the caption uses.
      rowTop: WINDOW.height - 640 / (SQUARE.width / (WINDOW.width - 10 - SIDEBAR_EDGE - 8)),
    }),
    wordmark: 128,
  },
};
