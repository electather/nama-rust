import { useTransport } from "@connectrpc/connect-query";

import { onServer } from "@/shared/lib/server";

/**
 * `url`, such as an image's the Server sent, on the Server the page's transport talks to: the Server
 * sends paths, which at the Hub the browser would ask the Hub for. An absolute URL, `""` and
 * `undefined` stay as they are.
 */
export function useServerUrl(url: string | undefined) {
  const transport = useTransport();
  return url?.startsWith("/") === true ? onServer(transport, url) : url;
}
