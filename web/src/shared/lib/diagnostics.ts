import type { Interceptor } from "@connectrpc/connect";
import type { AnyRouter } from "@tanstack/react-router";

// Nothing here imports OpenTelemetry, so the SDK is downloaded only when the Server exports
// Diagnostics (ADR 0054).
type Recorder = Awaited<ReturnType<typeof load>>;

// oxlint-disable-next-line vitest/require-hook -- module state, not test setup
let exporting = false;
let loading: Promise<Recorder> | undefined;
let recorder: Recorder | undefined;

function active() {
  return exporting ? recorder : undefined;
}

/**
 * Records the web app's Diagnostics from now on if the Server `exportsDiagnostics`, loading the SDK
 * only then. Only for a signed-in Device: the Server refuses anyone else's.
 */
export async function recordDiagnosticsIfExported(exportsDiagnostics: boolean) {
  exporting = exportsDiagnostics;
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
  const loaded = await import("@/shared/lib/diagnostics-recorder");
  addEventListener("error", (event) => active()?.recordError(kindOf(event.error)));
  addEventListener("unhandledrejection", (event) => active()?.recordError(kindOf(event.reason)));
  return loaded;
}

function kindOf(error: unknown) {
  return error instanceof Error ? error.name : typeof error;
}

/** Traces each ConnectRPC call while Diagnostics are recorded, sending its `traceparent`. */
export const traceCalls: Interceptor = (next) => async (request) => {
  const response = await (active()?.traceCall(next)(request) ?? next(request));
  return response;
};

// When the last route change started; 0, the page's own start, before any.
// oxlint-disable-next-line vitest/require-hook -- module state, not test setup
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
  active()?.recordSpan("playback start", playPressedAt);
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
  active()?.recordSpan("playback stall", stalledAt);
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
  active()?.recordError(kind, "playback error");
}
