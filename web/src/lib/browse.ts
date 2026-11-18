import {
  type DescMessage,
  type DescMethodUnary,
  type MessageInitShape,
  create,
} from "@bufbuild/protobuf";
import { Code } from "@connectrpc/connect";
import { createConnectQueryKey, createQueryOptions } from "@connectrpc/connect-query";
import { notFound } from "@tanstack/react-router";

import { BrowseService, type Track, type Version } from "@/gen/nama/v1/browse_pb";
import { LibraryService } from "@/gen/nama/v1/library_pb";
import { ExternalIdSchema } from "@/gen/nama/v1/match_pb";
import { isCode } from "@/shared/lib/connect-errors";
import { metadataLine } from "@/shared/lib/title-facts";
import { pictureOf, soundOf } from "@/shared/lib/version-facts";

import type { RouterContext } from "./access";

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
    if (isCode(error, Code.NotFound) || isCode(error, Code.InvalidArgument)) {
      throw notFound();
    }
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
  if (!library) {
    throw notFound();
  }
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

/** The id a `titleParam` names, after the router decodes it once; else the router's not-found. */
export function externalIdFromParam(param: string) {
  const [, namespace, value] = /^([a-z0-9]+)-(.+)$/u.exec(param) ?? [];
  try {
    if (namespace === undefined || value === undefined) {
      throw notFound();
    }
    return create(ExternalIdSchema, { namespace, value: decodeURIComponent(value) });
  } catch {
    throw notFound();
  }
}

/** Every browse query, including Continue Watching: what to refresh when Watch State changes. */
export const browseKey = createConnectQueryKey({ schema: BrowseService, cardinality: undefined });

const dates = new Intl.DateTimeFormat("en", { dateStyle: "medium", timeZone: "UTC" });

/** A `YYYY-MM-DD` date in plain words, such as "Feb 3, 2022". */
export function dateOf(date: string | undefined) {
  // A bare date parses as midnight UTC, so it's formatted in UTC too.
  return date === undefined ? undefined : dates.format(new Date(date));
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

/** The Version to play when nobody chose one: the largest file. */
function largestVersion(versions: Version[]) {
  // The Server sends at least one.
  return versions.reduce((largest, version) => (version.size > largest.size ? version : largest));
}

/** A whole number in the URL, such as a `$season`, or the router's not-found if it isn't one. */
export function wholeNumberFromParam(param: string) {
  if (!/^\d{1,15}$/u.test(param)) {
    throw notFound();
  }
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
