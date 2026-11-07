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
import { ExternalIdSchema } from "@/gen/nama/v1/match_pb";

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

/** A whole number in the URL, such as a `$libraryId`, or the router's not-found if it isn't one. */
export function wholeNumberFromParam(param: string) {
  if (!/^\d{1,15}$/u.test(param)) throw notFound();
  return Number(param);
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

/** The Version to play when nobody chose one: the largest file. */
function largestVersion(versions: Version[]) {
  // The Server sends at least one.
  return versions.reduce((largest, version) => (version.size > largest.size ? version : largest));
}

/** A player's search params: the Version `?version=` chose, if it names one by id. */
export function versionSearch({ version }: Record<string, unknown>): { version?: number } {
  return typeof version === "number" ? { version } : {};
}

/** The Version `chosen` names, or the largest when it names none of `versions`. */
export function versionToPlay(versions: Version[], chosen: number | undefined) {
  return versions.find(({ id }) => Number(id) === chosen) ?? largestVersion(versions);
}
