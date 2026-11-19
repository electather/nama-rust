import { useEffect, useRef } from "react";

import { startLightfall } from "@/shared/lib/lightfall/start-lightfall";

/** Streaks of light falling behind a signed-out page, which rush while `warp` is on. */
export function Lightfall({ warp }: { warp: boolean }) {
  const box = useRef<HTMLDivElement>(null);
  // Read every frame, so changing it doesn't restart the animation.
  const warping = useRef(warp);
  useEffect(() => {
    warping.current = warp;
  }, [warp]);
  useEffect(() => {
    // A canvas per start: stopping gives its WebGL context back, and a canvas never gets another.
    const canvas = document.createElement("canvas");
    canvas.className = "block size-full";
    box.current?.append(canvas);
    const stop = startLightfall(canvas, () => warping.current);
    return () => {
      stop?.();
      canvas.remove();
    };
  }, []);
  return (
    <div
      ref={box}
      aria-hidden
      className="pointer-events-none absolute inset-0 overflow-hidden bg-background"
    />
  );
}
