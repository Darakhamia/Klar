import type { ReactNode } from "react";
import type { Layout } from "../format";
import type { Shot } from "../motion";

/** Puts the stage point `shot.x, shot.y` in the middle of the frame, `shot.zoom` close. */
export function Camera({ layout, shot, children }: { layout: Layout; shot: Shot; children: ReactNode }) {
  const transform = [
    `translate(${String(layout.width / 2)}px, ${String(layout.height / 2)}px)`,
    `scale(${String(shot.zoom)})`,
    `translate(${String(-shot.x)}px, ${String(-shot.y)}px)`,
  ].join(" ");

  return (
    <div className="kv-world" style={{ width: layout.width, height: layout.height, transform }}>
      {children}
    </div>
  );
}
