import { createRouterTransport } from "@connectrpc/connect";
import { TransportProvider } from "@connectrpc/connect-query";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, renderHook, waitFor } from "@testing-library/react";
import { type ReactNode, Suspense } from "react";
import { expect, test } from "vite-plus/test";

import { RequestService } from "@/gen/nama/v1/request_pb";
import { usePaged } from "@/lib/paged";

/** A fake Server whose list has `count` pages, with each page's index as the token that lists it. */
function pagesOf(count: number) {
  return createRouterTransport((router) => {
    router.service(RequestService, {
      listRequests: ({ page }) => {
        const index = Number(page?.token ?? "");
        return {
          requests: [{ id: BigInt(index) }],
          page: {
            nextToken: index + 1 < count ? String(index + 1) : "",
            ...(index === 0 ? { totalSize: count } : {}),
          },
        };
      },
    });
  });
}

test("usePaged holds at most 10 pages and keeps the first page's total size after dropping it", async () => {
  const transport = pagesOf(12);
  const queryClient = new QueryClient();
  const wrapper = ({ children }: { children: ReactNode }) => (
    <TransportProvider transport={transport}>
      <QueryClientProvider client={queryClient}>
        <Suspense>{children}</Suspense>
      </QueryClientProvider>
    </TransportProvider>
  );
  const { result } = renderHook(
    () => usePaged(RequestService.method.listRequests, { page: { token: "" } }),
    { wrapper },
  );
  await waitFor(() => {
    expect(result.current.totalSize).toBe(12);
  });

  for (let fetched = 1; fetched < 12; fetched += 1) {
    // oxlint-disable-next-line no-await-in-loop -- each page asks with the token the one before gave
    await act(() => result.current.fetchNextPage());
  }

  const ids = result.current.data.pages.map((page) => page.requests[0]?.id);
  expect(ids).toEqual([2n, 3n, 4n, 5n, 6n, 7n, 8n, 9n, 10n, 11n]);
  expect(result.current.hasNextPage).toBe(false);
  expect(result.current.totalSize).toBe(12);
});
