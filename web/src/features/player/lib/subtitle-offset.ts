/** How far − and + shift a subtitle, in milliseconds. */
export const OFFSET_STEP_MS = 100;

/** A Subtitle Offset as the player shows it, in seconds to a tenth, such as "+0.5 s" or "−1.2 s". */
export function offsetText(offsetMs: number) {
  const seconds = `${(Math.abs(offsetMs) / 1000).toFixed(1)} s`;
  if (offsetMs === 0) {
    return seconds;
  }
  return `${offsetMs > 0 ? "+" : "−"}${seconds}`;
}

/**
 * The `timeOffset` JASSUB and libbitsub take for `offsetMs`: each draws the cue at the video's time
 * plus it, in seconds, so showing cues later draws those of an earlier time.
 */
export function drawnTimeOffset(offsetMs: number) {
  return -offsetMs / 1000;
}
