/**
 * Puts the pointer's place in its element, as the `--mx` and `--my` that `tile-sheen` reads. It
 * sets a style property and nothing else, so a pointer move costs no render.
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
  const { left, top } = currentTarget.getBoundingClientRect();
  currentTarget.style.setProperty("--mx", `${String(clientX - left)}px`);
  currentTarget.style.setProperty("--my", `${String(clientY - top)}px`);
}
