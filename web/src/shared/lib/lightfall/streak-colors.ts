const maxColors = 8;

/** `value`, any CSS colour, as linear-ish RGB: squared sRGB, as Lightfall approximates it. */
function toLinear(probe: CanvasRenderingContext2D, value: string) {
  probe.clearRect(0, 0, 1, 1);
  probe.fillStyle = value;
  probe.fillRect(0, 0, 1, 1);
  // `Array.from`, since mapping the clamped bytes in place would round each back to 0 or 1.
  return Array.from(probe.getImageData(0, 0, 1, 1).data.slice(0, 3), (c) => (c / 255) ** 2);
}

/**
 * The streaks' colours, from the tokens `element` sees: Near-white and Quiet ink, so they follow
 * the design system. A 2D canvas reads them, since it parses any CSS colour, oklch included.
 */
export function streakColors(element: Element) {
  const css = getComputedStyle(element);
  const probe = document.createElement("canvas").getContext("2d", { willReadFrequently: true });
  const read = (token: string) =>
    probe ? toLinear(probe, css.getPropertyValue(token).trim()) : [1, 1, 1];
  const colors = [read("--primary"), read("--muted-foreground")];
  const palette = new Float32Array(maxColors * 3);
  for (let i = 0; i < maxColors; i += 1) {
    palette.set(colors[Math.min(i, colors.length - 1)] ?? [], i * 3);
  }
  return { palette, top: new Float32Array(colors[0] ?? []) };
}
