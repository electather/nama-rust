import {
  type DescMessage,
  type DescMethodUnary,
  type MessageInitShape,
  fromJson,
} from "@bufbuild/protobuf";
import { Code } from "@connectrpc/connect";
import { createConnectQueryKey, createQueryOptions } from "@connectrpc/connect-query";
import { notFound } from "@tanstack/react-router";

import {
  BrowseService,
  HdrFormat,
  type Track,
  type Version,
  type VideoTrack,
} from "@/gen/nama/v1/browse_pb";
import { LibraryService } from "@/gen/nama/v1/library_pb";
import { type ExternalId, ExternalIdSchema } from "@/gen/nama/v1/match_pb";
import type { WatchState } from "@/gen/nama/v1/watch_state_pb";

import type { RouterContext } from "./access";
import { isCode } from "./connect-errors";

/**
 * Fetches `method` into the cache, unless it's there already. The Server's `not_found`, and the
 * `invalid_argument` it gives for a Library of the other kind, become the router's not-found page.
 */
export async function queryOrNotFound<I extends DescMessage, O extends DescMessage>(
  { queryClient, transport }: RouterContext,
  method: DescMethodUnary<I, O>,
  input: MessageInitShape<I>,
) {
  try {
    return await queryClient.query({
      ...createQueryOptions(method, input, { transport }),
      staleTime: "static",
    });
  } catch (error) {
    if (isCode(error, Code.NotFound) || isCode(error, Code.InvalidArgument)) throw notFound();
    throw error;
  }
}

/**
 * Whether a browse query's last refetch found its title gone, such as one deleted since. A failed
 * refetch keeps the data read before, so a page has to ask.
 */
export function isGone(error: unknown) {
  return isCode(error, Code.NotFound);
}

export async function libraryFromParam(context: RouterContext, param: string) {
  const { libraries } = await queryOrNotFound(context, LibraryService.method.listLibraries, {});
  const library = libraries.find(({ id }) => String(id) === param);
  if (!library) throw notFound();
  return library;
}

export async function loadMovie(
  context: RouterContext,
  params: { libraryId: string; movieId: string },
) {
  const library = await libraryFromParam(context, params.libraryId);
  const input = { libraryId: library.id, movieId: externalIdFromParam(params.movieId) };
  await queryOrNotFound(context, BrowseService.method.getMovie, input);
  return input;
}

/** How a Movie's or Show's external id is written in a URL, such as `tmdb-603`. */
export function titleParam(id: ExternalId | undefined) {
  return `${id?.id.case}-${id?.id.value}`;
}

/** The external id a `titleParam` in the URL names, or the router's not-found if it names none. */
export function externalIdFromParam(param: string) {
  const [, source = "", value] = /^([a-z]+)-(.+)$/u.exec(param) ?? [];
  try {
    // The JSON form, such as `{ "tmdb": "603" }`, refuses an unknown source or a malformed id.
    return fromJson(ExternalIdSchema, { [source]: value ?? "" });
  } catch {
    throw notFound();
  }
}

/** Every browse query, including Continue Watching: what to refresh when Watch State changes. */
export const browseKey = createConnectQueryKey({ schema: BrowseService, cardinality: undefined });

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

const dates = new Intl.DateTimeFormat("en", { dateStyle: "medium", timeZone: "UTC" });

/** A `YYYY-MM-DD` date in plain words, such as "Feb 3, 2022". */
export function dateOf(date: string | undefined) {
  // A bare date parses as midnight UTC, so it's formatted in UTC too.
  return date === undefined ? undefined : dates.format(new Date(date));
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
  const minutes = state ? Math.ceil(Number(state.durationMs - state.positionMs) / 60_000) : 0;
  return minutes > 0 ? `${runtimeOf(minutes, "short")} left` : undefined;
}

const gigabytes = new Intl.NumberFormat("en", {
  style: "unit",
  unit: "gigabyte",
  maximumFractionDigits: 1,
});

/** A file's or download's size, `bytes`, in gigabytes. */
export function sizeOf(bytes: bigint) {
  return gigabytes.format(Number(bytes) / 1e9);
}

/** Facts about a title, such as its year and runtime, joined into one line, skipping unknowns. */
export function metadataLine(...facts: (string | undefined)[]) {
  return facts.filter(Boolean).join(" · ");
}

/**
 * A Version's picture and sound in a few words, such as "4K · HDR · 5.1": its first video Track's
 * size and whether it's HDR, and its default audio Track's channels, or its first's. Empty until
 * the Server has probed it.
 */
export function versionLine({ tracks }: Version) {
  const [video] = tracks.flatMap(({ kind }) => (kind.case === "video" ? [kind.value] : []));
  const hdr = video && video.hdr !== HdrFormat.UNSPECIFIED ? "HDR" : undefined;
  return metadataLine(video && pictureOf(video), hdr, soundOf(tracks));
}

/**
 * A Track in a few words, such as "AAC · 5.1 · English · Commentary": its codec, picture size or
 * channels, language and title.
 */
export function trackLine(track: Track) {
  const { codec, language, title, kind } = track;
  return metadataLine(
    codecName(codec),
    kind.case === "video" ? pictureOf(kind.value) : soundOf([track]),
    language === undefined ? undefined : languageName(language),
    title,
  );
}

/** A codec, as ffprobe names it, by the name people know it by, or else in capitals. */
export function codecName(codec: string) {
  return codecNames.get(codec) ?? codec.toUpperCase();
}

const codecNames = new Map([
  ["h264", "H.264"],
  ["ac3", "Dolby Digital"],
  ["eac3", "Dolby Digital Plus"],
  ["truehd", "Dolby TrueHD"],
  ["dts", "DTS"],
  ["opus", "Opus"],
  ["vorbis", "Vorbis"],
  ["subrip", "SRT"],
  ["mov_text", "Text"],
  ["webvtt", "WebVTT"],
  ["hdmv_pgs_subtitle", "PGS"],
  ["dvd_subtitle", "VobSub"],
]);

const languageNames = new Intl.DisplayNames("en", { type: "language" });

/** A language code as the file tags it, such as `eng`, by name, or as it is if it isn't one. */
function languageName(code: string) {
  try {
    return languageNames.of(code) ?? code;
  } catch {
    // A file's tag can be anything, and `of` refuses what isn't a language code.
    return code;
  }
}

/** Picture sizes by name, largest first, with the width or height a picture reaches to be one. */
const pictureSizes = [
  // A film wider than 16:9 is shorter than its name, such as 3840×1600 for 4K, so width counts.
  { name: "4K", width: 3200, height: 2000 },
  { name: "1080p", width: 1800, height: 1000 },
  { name: "720p", width: 1200, height: 700 },
  { name: "SD", width: 1, height: 1 },
];

function pictureOf(video: VideoTrack) {
  return pictureSizes.find(({ width, height }) => video.width >= width || video.height >= height)
    ?.name;
}

const soundNames = new Map([
  [1, "Mono"],
  [2, "Stereo"],
  [6, "5.1"],
  [8, "7.1"],
]);

/**
 * The sound of `tracks`' default audio Track, or else its first, by its usual name, such as
 * "Stereo" or "5.1", or else how many channels it has.
 */
export function soundOf(tracks: Track[]) {
  const audio = tracks.flatMap(({ kind, default: isDefault }) =>
    kind.case === "audio" && kind.value.channels > 0
      ? [{ channels: kind.value.channels, isDefault }]
      : [],
  );
  const [first] = audio;
  const chosen = audio.find(({ isDefault }) => isDefault) ?? first;
  return chosen && channelsName(chosen.channels);
}

/** How many `channels` sound has, by its usual name, such as "Stereo" or "5.1". */
export function channelsName(channels: number) {
  return soundNames.get(channels) ?? `${channels} channels`;
}

/** The Version to play when nobody chose one: the largest file. */
function largestVersion(versions: Version[]) {
  // The Server sends at least one.
  return versions.reduce((largest, version) => (version.size > largest.size ? version : largest));
}

/** A whole number in the URL, such as a `$season`, or the router's not-found if it isn't one. */
export function wholeNumberFromParam(param: string) {
  if (!/^\d{1,15}$/u.test(param)) throw notFound();
  return Number(param);
}

/** What a page's URL says a delete left it to report: what went, and the Fulfillers not told. */
export interface DeletedSearch {
  deleted?: string | undefined;
  notTold?: string[] | undefined;
}

/**
 * The search that reports deleting `deleted` when the Fulfillers `notTold` couldn't be told, or
 * clears an earlier report when every one was.
 */
export function deletedSearch(deleted: string, notTold: string[]): DeletedSearch {
  return notTold.length > 0 ? { deleted, notTold } : { deleted: undefined, notTold: undefined };
}

/** The report of a delete in `search`, as `deletedSearch` wrote it. */
export function deletedFromSearch({ deleted, notTold }: Record<string, unknown>): DeletedSearch {
  // Set even when unknown: the router keeps the raw value under any key this leaves out.
  const unknown = { deleted: undefined, notTold: undefined };
  return typeof deleted === "string" && isNames(notTold) ? { deleted, notTold } : unknown;
}

function isNames(value: unknown): value is string[] {
  return Array.isArray(value) && value.every((name) => typeof name === "string");
}

/** A player's search params: the Version `?version=` chose, if it names one by id. */
export function versionSearch({ version }: Record<string, unknown>): { version?: number } {
  return typeof version === "number" ? { version } : {};
}

/** The Version `chosen` names, or the largest when it names none of `versions`. */
export function versionToPlay(versions: Version[], chosen?: number) {
  return versions.find(({ id }) => Number(id) === chosen) ?? largestVersion(versions);
}
