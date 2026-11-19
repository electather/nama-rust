import { createRenderer, type Frame } from "@/shared/lib/lightfall/renderer";

const introSeconds = 2.6;
const warpSpeed = 9;

/** How `frame` moves on over `dt` seconds, warping or not. */
function advance(frame: Frame, dt: number, warping: boolean) {
  frame.intro = Math.min(1, frame.intro + dt / introSeconds);
  // Eases into a warp at 2.2/s and back out at 1.6/s, as Lightfall's hold-to-warp does.
  const target = warping ? 1 : 0;
  frame.warp += (target - frame.warp) * (1 - Math.exp(-dt * (warping ? 2.2 : 1.6)));
  const rush = warpSpeed * (1 - frame.intro) ** 2 + 6 * frame.warp;
  frame.travel += dt * 0.03 * (1 + rush);
}

/**
 * Starts the sign-in pages' streaks of light on `canvas`, reading `isWarping()` each frame. With
 * reduced motion it draws one still frame. Returns a function that stops it and frees the GPU, or
 * `null` where WebGL2 can't draw it.
 */
export function startLightfall(canvas: HTMLCanvasElement, isWarping: () => boolean) {
  let renderer: ReturnType<typeof createRenderer>;
  try {
    renderer = createRenderer(canvas);
  } catch {
    // No WebGL2, or a driver that can't run the shaders: the page's plain background stays.
    return null;
  }
  const still = globalThis.matchMedia("(prefers-reduced-motion: reduce)").matches;
  const frame: Frame = { travel: 0, intro: still ? 1 : 0, warp: 0 };
  let raf = 0;
  let last = 0;

  const tick = (now: number) => {
    raf = 0;
    if (document.hidden) {
      return;
    }
    const dt = last > 0 ? Math.min(0.05, (now - last) / 1000) : 1 / 60;
    last = now;
    advance(frame, dt, isWarping());
    renderer.render(frame);
    raf = requestAnimationFrame(tick);
  };
  const wake = () => {
    if (!still && raf === 0 && !document.hidden) {
      last = 0;
      raf = requestAnimationFrame(tick);
    }
  };
  const resize = () => {
    const ratio = Math.min(globalThis.devicePixelRatio, 2);
    canvas.width = Math.max(1, Math.round(canvas.clientWidth * ratio));
    canvas.height = Math.max(1, Math.round(canvas.clientHeight * ratio));
    renderer.render(frame);
    wake();
  };
  const observer = new ResizeObserver(resize);
  observer.observe(canvas);
  document.addEventListener("visibilitychange", wake);

  return () => {
    cancelAnimationFrame(raf);
    observer.disconnect();
    document.removeEventListener("visibilitychange", wake);
    renderer.dispose();
  };
}
