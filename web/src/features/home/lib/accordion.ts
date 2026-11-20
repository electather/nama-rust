/** How much of the hero's width the open panel takes: the artboard's 52%. */
const OPEN_SHARE = 0.52;

/** How far a closed panel turns away from the open one, in degrees. */
const TILT = 8;

/** How far, in `cqw`, a closed panel's art drifts per panel away from the open one. */
const DRIFT = 1.9;

/** The open panel's `flex-grow` among `count` panels, the closed ones growing by 1 each. */
export function openGrow(count: number) {
  return count > 1 ? (OPEN_SHARE * (count - 1)) / (1 - OPEN_SHARE) : 1;
}

/** A panel `offset` panels from the open one turns toward it; the open one faces you. */
export function tiltOf(offset: number) {
  if (offset === 0) {
    return "none";
  }
  return `rotateY(${offset < 0 ? TILT : -TILT}deg)`;
}

/** How far a panel's art drifts from its centre, so neighbours read as one sweep. */
export function driftOf(offset: number) {
  return `${(Math.max(-1.5, Math.min(1.5, offset)) * DRIFT).toFixed(2)}cqw`;
}
