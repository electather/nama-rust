import { useTransport } from "@connectrpc/connect-query";

import { onServer } from "@/shared/lib/server";

/**
 * `url`, a URL the Server sent, on the Server the page's transport talks to: the Server sends
 * paths, which at the Hub the browser would ask the Hub for. An image shows with `ServerImg`, which
 * calls this for it. An absolute or protocol-relative URL, `""` and `undefined` stay as they are.
 */
export function useServerUrl(url: string): string;
export function useServerUrl(url: string | undefined): string | undefined;
export function useServerUrl(url: string | undefined) {
  const transport = useTransport();
  return url === undefined ? url : onServer(transport, url);
}
