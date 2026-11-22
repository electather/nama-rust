const HOUR = 3600;
const DAY = 24 * HOUR;

type Step = [limit: number, size: number, unit: Intl.RelativeTimeFormatUnit];

const YEARS: Step = [Infinity, 365 * DAY, "year"];
const STEPS: Step[] = [
  [DAY, HOUR, "hour"],
  [30 * DAY, DAY, "day"],
  [365 * DAY, 30 * DAY, "month"],
];

/**
 * When a Device was last used, `seconds` since the epoch, as a relative time such as "Last used 2
 * days ago". The Server keeps it to the hour, so within one it's "Active now".
 */
export function lastUsed(seconds: bigint, now = Date.now()) {
  const ago = Math.max(0, now / 1000 - Number(seconds));
  if (ago < HOUR) {
    return "Active now";
  }
  const [, size, unit] = STEPS.find(([limit]) => ago < limit) ?? YEARS;
  const words = new Intl.RelativeTimeFormat(undefined, { numeric: "auto" });
  return `Last used ${words.format(-Math.floor(ago / size), unit)}`;
}
