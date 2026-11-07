import {
  type DescMessage,
  type DescMethodUnary,
  type MessageInitShape,
  fromJson,
} from "@bufbuild/protobuf";
import { Code } from "@connectrpc/connect";
import { createQueryOptions } from "@connectrpc/connect-query";
import { notFound } from "@tanstack/react-router";

import type { Version } from "@/gen/nama/v1/browse_pb";
import { LibraryService } from "@/gen/nama/v1/library_pb";
import { type ExternalId, ExternalIdSchema } from "@/gen/nama/v1/match_pb";

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

/** The Library a `$libraryId` in the URL names, or the router's not-found if there's none. */
export async function libraryFromParam(context: RouterContext, param: string) {
  const { libraries } = await queryOrNotFound(context, LibraryService.method.listLibraries, {});
  const library = libraries.find(({ id }) => String(id) === param);
  if (!library) throw notFound();
  return library;
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

/** The year in a `YYYY-MM-DD` date. */
export function yearOf(date: string | undefined) {
  return date?.slice(0, 4);
}

const dates = new Intl.DateTimeFormat("en", { dateStyle: "medium", timeZone: "UTC" });

/** A `YYYY-MM-DD` date in plain words, such as "Feb 3, 2022". */
export function dateOf(date: string | undefined) {
  // A bare date parses as midnight UTC, so it's formatted in UTC too.
  return date === undefined ? undefined : dates.format(new Date(date));
}

/** A runtime in plain words, such as "2 h 16 min", or nothing when it isn't known. */
export function runtimeOf(minutes = 0) {
  const hours = Math.floor(minutes / 60);
  const rest = minutes % 60;
  return [hours > 0 ? `${hours} h` : "", rest > 0 ? `${rest} min` : ""].filter(Boolean).join(" ");
}

/** Facts about a title, such as its year and runtime, joined into one line, skipping unknowns. */
export function metadataLine(...facts: (string | undefined)[]) {
  return facts.filter(Boolean).join(" · ");
}

/** The Version to play when nobody chose one: the largest file. */
export function largestVersion(versions: Version[]) {
  // The Server sends at least one.
  return versions.reduce((largest, version) => (version.size > largest.size ? version : largest));
}

/** A whole number in the URL, such as a `$season`, or the router's not-found if it isn't one. */
export function wholeNumberFromParam(param: string) {
  if (!/^\d{1,15}$/u.test(param)) throw notFound();
  return Number(param);
}

/** A player's search params: the Version `?version=` chose, if it names one by id. */
export function versionSearch({ version }: Record<string, unknown>): { version?: number } {
  return typeof version === "number" ? { version } : {};
}

/** The Version `chosen` names, or the largest when it names none of `versions`. */
export function versionToPlay(versions: Version[], chosen: number | undefined) {
  return versions.find(({ id }) => Number(id) === chosen) ?? largestVersion(versions);
}
