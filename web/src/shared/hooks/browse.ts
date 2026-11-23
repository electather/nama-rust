import {
  type DescMessage,
  type DescMethodUnary,
  type MessageInitShape,
  create,
} from "@bufbuild/protobuf";
import { Code } from "@connectrpc/connect";
import { createConnectQueryKey, createQueryOptions } from "@connectrpc/connect-query";
import { notFound } from "@tanstack/react-router";

import { BrowseService } from "@/gen/nama/v1/browse_pb";
import { LibraryService } from "@/gen/nama/v1/library_pb";
import { ExternalIdSchema } from "@/gen/nama/v1/match_pb";
import type { RouterContext } from "@/shared/hooks/access";
import { isCode } from "@/shared/lib/connect-errors";

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
  const input = {
    libraryId: libraryIdFromParam(params.libraryId),
    movieId: externalIdFromParam(params.movieId),
  };
  await queryOrNotFound(context, BrowseService.method.getMovie, input);
  return input;
}

/** The id a `titleParam` names, after the router decodes it once; else the router's not-found. */
export function externalIdFromParam(param: string) {
  const { namespace, value } = /^(?<namespace>[a-z0-9]+)-(?<value>.+)$/u.exec(param)?.groups ?? {};
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

/**
 * The Library id in the URL, or the router's not-found if it isn't one. A title's RPC answers
 * `not_found` for a Library the Server lacks, so it needn't wait for the Libraries.
 */
export function libraryIdFromParam(param: string) {
  return BigInt(wholeNumberFromParam(param));
}

/** A whole number in the URL, such as a `$season`, or the router's not-found if it isn't one. */
export function wholeNumberFromParam(param: string) {
  if (!/^\d{1,15}$/u.test(param)) {
    throw notFound();
  }
  return Number(param);
}
