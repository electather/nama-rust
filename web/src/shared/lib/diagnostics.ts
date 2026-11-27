import type { Interceptor, Transport } from "@connectrpc/connect";
import type { AnyRouter } from "@tanstack/react-router";

import { originOf } from "@/shared/lib/server";

// Nothing here imports OpenTelemetry, so the SDK is downloaded only when a Server exports
// Diagnostics (ADR 0054).
type Sdk = Awaited<ReturnType<typeof load>>;
type Recorder = ReturnType<Sdk["recorderAt"]>;

// The headers each Server that exports Diagnostics takes them with, by origin, `""` for the page's
// own: at the Hub each Server's admin decides for that Server's alone (ADR 0054, ADR 0105).
// oxlint-disable-next-line vitest/require-hook -- module state, not test setup
const exporting = new Map<string, Record<string, string>>();
const recorders = new Map<string, Recorder>();
let loading: Promise<Sdk> | undefined;
let sdk: Sdk | undefined;
// The origin of the Server the current route shows. On the Hub's own pages it's the Hub's, which
// exports nothing, so their spans are dropped.
// oxlint-disable-next-line vitest/require-hook -- module state, not test setup
let routeOrigin = "";

/** The recorder for the Server at `origin`, made the first time, while that Server exports. */
function recorderFor(origin: string) {
  const headers = exporting.get(origin);
  if (sdk && headers && !recorders.has(origin)) {
    // Asked at each post, so it posts with the session the page signed in with last.
    recorders.set(
      origin,
      sdk.recorderAt(origin, () => exporting.get(origin) ?? headers),
    );
  }
  return headers ? recorders.get(origin) : undefined;
}

function routeRecorder() {
  return recorderFor(routeOrigin);
}

/**
 * Records the web app's Diagnostics for the Server at `origin`, `""` for the page's own, from now
 * on if it `exportsDiagnostics`, loading the SDK only then. Only for a Device signed in there, with
 * its session `token` at the Hub: the Server refuses anyone else's.
 */
export async function recordDiagnosticsIfExported(
  exportsDiagnostics: boolean,
  origin = "",
  token?: string,
) {
  if (!exportsDiagnostics) {
    exporting.delete(origin);
    return;
  }
  exporting.set(origin, token === undefined ? {} : { authorization: `Bearer ${token}` });
  loading ??= load();
  try {
    sdk = await loading;
  } catch {
    // Diagnostics are never worth breaking the page for, such as when a deploy removed the chunk.
  }
}

async function load() {
  const loaded = await import("@/shared/lib/diagnostics-recorder");
  addEventListener("error", (event) => routeRecorder()?.recordError(kindOf(event.error)));
  addEventListener("unhandledrejection", (event) =>
    routeRecorder()?.recordError(kindOf(event.reason)),
  );
  return loaded;
}

function kindOf(error: unknown) {
  return error instanceof Error ? error.name : typeof error;
}

/**
 * Traces each ConnectRPC call to the Server at `origin`, `""` for the page's own, while it exports
 * Diagnostics, sending its `traceparent`.
 */
export function traceCallsTo(origin: string): Interceptor {
  return (next) => async (request) => {
    const response = await (recorderFor(origin)?.traceCall(next)(request) ?? next(request));
    return response;
  };
}

// When the last route change started; 0, the page's own start, before any.
// oxlint-disable-next-line vitest/require-hook -- module state, not test setup
let navigatedAt = 0;

/** Records each route change, from leaving one page to the next one's data having loaded. */
export function traceRouteChanges(router: AnyRouter) {
  router.subscribe("onBeforeNavigate", () => {
    navigatedAt = performance.now();
  });
  router.subscribe("onResolved", () => {
    const page = router.state.matches.at(-1);
    // At the Hub a Server's pages put the transport to it in their context (ADR 0105). A page that
    // threw `notFound` before its context was made has none, whatever its type says.
    const transport: Transport | undefined = page?.context.transport;
    routeOrigin = transport ? originOf(transport) : "";
    // The route's id, such as `/_member/libraries/$libraryId`, names the page without its ids.
    const route = page?.routeId ?? "unknown";
    routeRecorder()?.recordSpan("route change", navigatedAt, { "app.route": route });
  });
}

let playPressedAt: number | undefined;
// When the playing `<video>` began waiting for more of the video; undefined while it isn't.
let stalledAt: number | undefined;

/**
 * Marks play as pressed at the route change that opened the player, so the player's own loading
 * counts too.
 */
export function playerOpened() {
  // ponytail: where the browser blocks autoplay, the span also holds the wait for the User to press
  // play in the player; drop it then if that skews playback start.
  playPressedAt = navigatedAt;
  stalledAt = undefined;
}

/** Records how long the first frame took since play was pressed, once per press. */
export function firstFramePlayed() {
  if (playPressedAt === undefined) {
    return;
  }
  routeRecorder()?.recordSpan("playback start", playPressedAt);
  playPressedAt = undefined;
}

/**
 * Marks a stall starting as the `<video>` waits, unless its first frame is yet to play or the wait
 * is a seek's own.
 */
export function stallStarted({ seeking }: Pick<HTMLMediaElement, "seeking">) {
  // ponytail: a pause or seek mid-stall counts until it plays again; end the stall on `pause` if
  // that skews it.
  if (playPressedAt !== undefined || seeking) {
    return;
  }
  stalledAt ??= performance.now();
}

/** Records the stall, if any, as the `<video>` plays again. */
export function stallEnded() {
  if (stalledAt === undefined) {
    return;
  }
  routeRecorder()?.recordSpan("playback stall", stalledAt);
  stalledAt = undefined;
}

// A `MediaError`'s names by its `code`, from 1.
const mediaErrors = [
  "MEDIA_ERR_ABORTED",
  "MEDIA_ERR_NETWORK",
  "MEDIA_ERR_DECODE",
  "MEDIA_ERR_SRC_NOT_SUPPORTED",
];

/** Records the `<video>`'s `error` as a playback error, by its code's name. */
export function videoFailed(error: MediaError | null) {
  playbackFailed(mediaErrors[(error?.code ?? 0) - 1] ?? "unknown");
}

/** Records a playback error of `kind`, such as hls.js's `bufferStalledError`, never its message. */
export function playbackFailed(kind: string) {
  routeRecorder()?.recordError(kind, "playback error");
}
