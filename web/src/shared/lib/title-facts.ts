import type { WatchState } from "@/gen/nama/v1/watch_state_pb";

// Facts about a title in plain words, for its card, page or row.

/** A Season's name: "Season 2", or "Specials" for Season 0. */
export function seasonName(number: number) {
  return number === 0 ? "Specials" : `Season ${number}`;
}

/**
 * An Episode's name: "Season 2, Episode 3" in full, "S2, E3" short or "Episode 3" in its Season. A
 * Special says "Specials" in every form, in its Season too, as the Delete dialog names it.
 */
export function episodeName(season: number, episode: number, form: "full" | "short" | "inSeason") {
  const full = `${seasonName(season)}, Episode ${episode}`;
  const short = `${season === 0 ? seasonName(season) : `S${season}`}, E${episode}`;
  return { full, short, inSeason: season === 0 ? full : `Episode ${episode}` }[form];
}

export function yearOf(date: string | undefined) {
  return date?.slice(0, 4);
}

/** The day `seconds` since the Unix epoch falls on, as this browser writes dates. */
export function dayOf(seconds: bigint) {
  return new Date(Number(seconds) * 1000).toLocaleDateString(undefined, { dateStyle: "medium" });
}

/**
 * A runtime in whole hours and minutes, each left out when 0: in plain words, such as "2 h 16 min",
 * or short, "2h 16m". Nothing when it isn't known.
 */
export function runtimeOf(minutes = 0, form: "long" | "short" = "long") {
  const [hourUnit, minuteUnit] = form === "long" ? [" h", " min"] : ["h", "m"];
  const hours = Math.floor(minutes / 60);
  const rest = minutes % 60;
  return [hours > 0 ? `${hours}${hourUnit}` : "", rest > 0 ? `${rest}${minuteUnit}` : ""]
    .filter(Boolean)
    .join(" ");
}

/** What's left of a started Movie or Episode, such as "1h 12m left", or nothing if unknown. */
export function timeLeft(state: WatchState | undefined) {
  const left = remainingOf(state);
  return left === undefined ? undefined : `${left} left`;
}

/** How long a started Movie or Episode has left, such as "1h 12m", or nothing if unknown. */
export function remainingOf(state: WatchState | undefined) {
  const minutes = state ? Math.ceil(Number(state.durationMs - state.positionMs) / 60_000) : 0;
  return minutes > 0 ? runtimeOf(minutes, "short") : undefined;
}

function sizeUnit(bytesPerUnit: number, unit: "terabyte" | "gigabyte" | "megabyte" | "kilobyte") {
  const formatter = new Intl.NumberFormat("en", { style: "unit", unit, maximumFractionDigits: 1 });
  // 999.95 of the unit below rounds to "1,000", so from there a size reads in this unit.
  return { roundsUpFrom: bytesPerUnit * 0.99995, bytesPerUnit, formatter };
}

const kilobytes = sizeUnit(1e3, "kilobyte");
const unitsLargestFirst = [
  sizeUnit(1e12, "terabyte"),
  sizeUnit(1e9, "gigabyte"),
  sizeUnit(1e6, "megabyte"),
];

/** A file's or download's size, `bytes`, in the largest unit up to TB it fills, as a file manager shows it. */
export function sizeOf(bytes: bigint) {
  const size = Number(bytes);
  const { bytesPerUnit, formatter } =
    unitsLargestFirst.find((unit) => size >= unit.roundsUpFrom) ?? kilobytes;
  return formatter.format(size / bytesPerUnit);
}

/** Facts about a title, such as its year and runtime, joined into one line, skipping unknowns. */
export function metadataLine(...facts: (string | undefined)[]) {
  return facts.filter(Boolean).join(" · ");
}
