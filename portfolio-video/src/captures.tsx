/**
 * The captures of the real app, and how the video shows them.
 *
 * `public/captures/` is written by capture/capture.mjs: PNGs of the app's own
 * frontend at DPR 6, and a manifest with the boxes (in CSS pixels of the
 * captured page) that the video crops to and underlines.
 */

import { createContext, useContext, type CSSProperties } from "react";
import { Img, staticFile } from "remotion";

export interface Box {
  x: number;
  y: number;
  width: number;
  height: number;
}

/** A captured region of a page: its file, and where it sat on the page in CSS pixels. */
interface Page {
  file: string;
  dpr: number;
  x: number;
  y: number;
  width: number;
  height: number;
}

export interface Manifest {
  voice: Page & {
    boxes: { row: Box; label: Box; hint: Box; figure: Box; sentence: Box[] };
  };
  dictionary: Page & {
    boxes: { tryField: Box; preview: Box; heard: Box; term: Box; list: Box };
  };
  pill: {
    dir: string;
    dpr: number;
    fps: number;
    frames: number;
    listening: { from: number; to: number };
    done: { from: number; to: number };
    /** CSS height of every frame. */
    height: number;
    /** CSS width of each frame. */
    widths: number[];
  };
}

export const ManifestContext = createContext<Manifest | null>(null);

export function useManifest(): Manifest {
  const manifest = useContext(ManifestContext);
  if (manifest === null) throw new Error("captures/manifest.json has not loaded");
  return manifest;
}

/** The union of boxes, grown by `pad` CSS pixels on every side. */
export function around(boxes: Box[], pad: number): Box {
  const left = Math.min(...boxes.map((b) => b.x)) - pad;
  const top = Math.min(...boxes.map((b) => b.y)) - pad;
  const right = Math.max(...boxes.map((b) => b.x + b.width)) + pad;
  const bottom = Math.max(...boxes.map((b) => b.y + b.height)) + pad;
  return { x: left, y: top, width: right - left, height: bottom - top };
}

/**
 * A window onto a captured page: `crop`, in the page's CSS pixels, shown at
 * `scale` frame pixels per CSS pixel. Never above the capture's DPR, so the
 * source is never enlarged. `crop` must lie inside the captured region.
 */
export function Crop({
  page,
  crop,
  scale,
  style,
  children,
}: {
  page: Page;
  crop: Box;
  scale: number;
  style?: CSSProperties;
  children?: React.ReactNode;
}) {
  return (
    <div
      style={{
        position: "relative",
        width: crop.width * scale,
        height: crop.height * scale,
        overflow: "hidden",
        ...style,
      }}
    >
      <Img
        src={staticFile(`captures/${page.file}`)}
        style={{
          position: "absolute",
          left: (page.x - crop.x) * scale,
          top: (page.y - crop.y) * scale,
          width: page.width * scale,
          height: page.height * scale,
          maxWidth: "none",
        }}
      />
      {children}
    </div>
  );
}

/** Where a box of the page lands inside a `Crop`, in frame pixels. */
export const within = (box: Box, crop: Box, scale: number): Box => ({
  x: (box.x - crop.x) * scale,
  y: (box.y - crop.y) * scale,
  width: box.width * scale,
  height: box.height * scale,
});
