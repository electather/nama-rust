import type { Interceptor, Transport } from "@connectrpc/connect";
import { createConnectTransport } from "@connectrpc/connect-web";

/**
 * The transport to the Server at `origin`, such as `https://nama.example.com`, or `""` for the
 * Server that served the page. `onServer` reads `origin` back to put the Server's own paths on it.
 */
export function serverTransport(origin: string, interceptors: Interceptor[]) {
  return { ...createConnectTransport({ baseUrl: `${origin}/`, interceptors }), origin };
}

/**
 * `path`, such as `/custom-images/movie/…`, on the Server `transport` talks to. A transport
 * `serverTransport` didn't make, such as a test's fake Server, is to the page's own.
 */
export function onServer(transport: Transport, path: string) {
  return "origin" in transport && typeof transport.origin === "string"
    ? `${transport.origin}${path}`
    : path;
}
