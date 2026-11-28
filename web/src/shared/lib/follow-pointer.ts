/** The most a tile leans toward the pointer at its edge, in degrees (DESIGN.md, "Card motion"). */
const TILT_DEG = 3;

/** `value` as a share of `size`, centred on 0 and kept within ±0.5. */
function centred(value: number, size: number) {
  return Math.min(Math.max(value / size, 0), 1) - 0.5;
}

/**
 * Puts the pointer's place in its element: `--mx` and `--my` for `tile-sheen`, and the lean for
 * `tile-tilt`. It sets style properties and nothing else, so a
 * pointer move costs no render.
 */
export function followPointer({
  currentTarget,
  clientX,
  clientY,
}: {
  currentTarget: HTMLElement;
  clientX: number;
  clientY: number;
}) {
  const { left, top, width, height } = currentTarget.getBoundingClientRect();
  const x = centred(clientX - left, width);
  const y = centred(clientY - top, height);
  const { style } = currentTarget;
  style.setProperty("--mx", `${String(clientX - left)}px`);
  style.setProperty("--my", `${String(clientY - top)}px`);
  // The edge under the pointer dips away, as if the pointer pressed it.
  style.setProperty("--tilt-x", `${String(-y * 2 * TILT_DEG)}deg`);
  style.setProperty("--tilt-y", `${String(x * 2 * TILT_DEG)}deg`);
}
