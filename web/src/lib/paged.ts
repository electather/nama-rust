import {
  type DescMessage,
  type DescMethodUnary,
  type MessageInitShape,
  isMessage,
} from "@bufbuild/protobuf";
import type { Transport } from "@connectrpc/connect";
import { createInfiniteQueryOptions, useTransport } from "@connectrpc/connect-query";
import { useSuspenseInfiniteQuery } from "@tanstack/react-query";
import { useState } from "react";

import {
  type PageRequestSchema,
  type PageResponse,
  PageResponseSchema,
} from "@/gen/nama/v1/page_pb";

/**
 * How a paged list's infinite query pages: by `page.token`, holding at most 10 pages, so at most
 * 1,000 rows are held and an event's invalidation refetches at most 10 pages.
 */
const paging = {
  pageParamKey: "page.token",
  getNextPageParam: ({ page }: { page?: PageResponse | undefined }) =>
    page === undefined || page.nextToken === "" ? undefined : page.nextToken,
  maxPages: 10,
} as const;

/** The query `usePaged` reads `method`'s list for `input` with, for a route's loader to prefetch. */
export function pagedQuery<I extends DescMessage, O extends DescMessage>(
  method: DescMethodUnary<I, O>,
  input: MessageInitShape<I> & { page: MessageInitShape<typeof PageRequestSchema> },
  transport: Transport,
) {
  const { maxPages, ...connectPaging } = paging;
  return {
    // oxlint-disable-next-line typescript/no-unsafe-type-assertion -- connect-query can't see that a generic `I` has the `page.token` `input`'s type promises
    ...createInfiniteQueryOptions(method, input as never, { transport, ...connectPaging } as never),
    maxPages,
  };
}

/**
 * The pages of `method`'s list for `input` so far, as `useSuspenseInfiniteQuery` gives them, and
 * `totalSize`, which only the first page carries, kept once `maxPages` drops that page.
 */
export function usePaged<I extends DescMessage, O extends DescMessage>(
  method: DescMethodUnary<I, O>,
  input: MessageInitShape<I> & { page: MessageInitShape<typeof PageRequestSchema> },
) {
  const query = useSuspenseInfiniteQuery(pagedQuery(method, input, useTransport()));
  const counted = totalSizeIn(query.data.pages[0]);
  const [totalSize, setTotalSize] = useState(counted);
  if (counted !== undefined && counted !== totalSize) {
    setTotalSize(counted);
  }
  return { ...query, totalSize };
}

/** The `total_size` `response`'s `PageResponse` carries, which only a first page has. */
function totalSizeIn(response: object | undefined) {
  const page = response !== undefined && "page" in response ? response.page : undefined;
  return isMessage(page, PageResponseSchema) ? page.totalSize : undefined;
}
