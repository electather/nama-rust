/** The slowest the player plays: Firefox mutes the sound below it. */
export const SLOWEST = 0.25;
/** The fastest the player plays: Firefox mutes the sound above it. */
export const FASTEST = 4;
/** How finely the slider and − and + set the speed. */
export const SPEED_STEP = 0.05;
/** How far < and > step the speed, as YouTube's do. */
const KEY_STEP = 0.25;
/** The speeds a tap sets, as YouTube offers them. */
export const PRESETS = [1, 1.25, 1.5, 2, 3];

/** A speed by name, such as "1.5×", or "Normal". */
export function speedName(rate: number) {
  return rate === 1 ? "Normal" : `${rate}×`;
}

/** A speed to two places, as the speed view shows it, such as "1.00×". */
export function speedValue(rate: number) {
  return `${rate.toFixed(2)}×`;
}

/** A preset's label, such as "1.0" or "1.25". */
export function presetName(rate: number) {
  return Number.isInteger(rate) ? rate.toFixed(1) : String(rate);
}

/** `rate` between `SLOWEST` and `FASTEST`, to the hundredth, as adding steps drifts. */
export function clampSpeed(rate: number) {
  return Math.min(FASTEST, Math.max(SLOWEST, Math.round(rate * 100) / 100));
}

/** The speed < (`by` -1) or > (`by` 1) goes to: the next quarter down or up from `rate`. */
export function keyStep(rate: number, by: 1 | -1) {
  const quarters = by > 0 ? Math.floor(rate / KEY_STEP) : Math.ceil(rate / KEY_STEP);
  return clampSpeed((quarters + by) * KEY_STEP);
}
