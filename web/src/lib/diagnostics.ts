import type { Interceptor } from "@connectrpc/connect";
import type { AnyRouter } from "@tanstack/react-router";

import { type RouterContext, serverInfo } from "./access";

// Nothing here imports OpenTelemetry, so the SDK is downloaded only when the Server exports
// Diagnostics (ADR 0054).
type Recorder = Awaited<ReturnType<typeof load>>;

let exporting = false;
let loading: Promise<Recorder> | undefined;
let recorder: Recorder | undefined;

function active() {
  return exporting ? recorder : undefined;
}

/**
 * Records the web app's Diagnostics from now on if the Server exports them, loading the SDK only
 * then. Only for a signed-in Device: the Server refuses anyone else's.
 */
export async function recordDiagnosticsIfExported(context: RouterContext) {
  exporting = (await serverInfo(context)).exportsDiagnostics;
  if (!exporting) {
    return;
  }
  loading ??= load();
  try {
    recorder = await loading;
  } catch {
    // Diagnostics are never worth breaking the page for, such as when a deploy removed the chunk.
  }
}

async function load() {
  const loaded = await import("./diagnostics-recorder");
  addEventListener("error", (event) => active()?.recordError(kindOf(event.error)));
  addEventListener("unhandledrejection", (event) => active()?.recordError(kindOf(event.reason)));
  return loaded;
}

function kindOf(error: unknown) {
  return error instanceof Error ? error.name : typeof error;
}

/** Traces each ConnectRPC call while Diagnostics are recorded, sending its `traceparent`. */
export const traceCalls: Interceptor = (next) => (request) =>
  active()?.traceCall(next)(request) ?? next(request);

// When the last route change started; 0, the page's own start, before any.
let navigatedAt = 0;

/** Records each route change, from leaving one page to the next one's data having loaded. */
export function traceRouteChanges(router: AnyRouter) {
  router.subscribe("onBeforeNavigate", () => {
    navigatedAt = performance.now();
  });
  router.subscribe("onResolved", () => {
    // The route's id, such as `/_member/libraries/$libraryId`, names the page without its ids.
    const route = router.state.matches.at(-1)?.routeId ?? "unknown";
    active()?.recordSpan("route change", navigatedAt, { "app.route": route });
  });
}

let playPressedAt: number | undefined;

/**
 * Marks play as pressed at the route change that opened the player, so the player's own loading
 * counts too.
 */
export function playerOpened() {
  // ponytail: where the browser blocks autoplay, the span also holds the wait for the User to press
  // play in the player; drop it then if that skews playback start.
  playPressedAt = navigatedAt;
}

/** Records how long the first frame took since play was pressed, once per press. */
export function firstFramePlayed() {
  if (playPressedAt === undefined) {
    return;
  }
  active()?.recordSpan("playback start", playPressedAt);
  playPressedAt = undefined;
}
