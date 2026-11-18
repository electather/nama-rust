import type { PlaybackSubtitle } from "@/gen/nama/v1/playback_pb";

/** How a subtitle turned on is getting on: being read out of the file, shown, or not to be had. */
export type SubtitleState = "loading" | "ready" | "failed";

/** A renderer drawing a subtitle over a `<video>` until it's disposed of. */
export interface Renderer {
  dispose: () => void;
}

/** What a renderer draws, over what, and how it says it's getting on. */
export interface RenderRequest {
  video: HTMLVideoElement;
  subtitle: PlaybackSubtitle;
  ready: () => void;
  failed: () => void;
}

/**
 * Makes a renderer drawing the request's subtitle over its video, calling `ready` once it draws,
 * or `failed` if it won't start or throws. `null` when what it draws can't be fetched.
 */
export type Render = (request: RenderRequest) => Promise<Renderer | null>;

/** What `draw` draws, and how it hears of a renderer that won't start or throws. */
export interface DrawRequest {
  video: HTMLVideoElement;
  subtitle: PlaybackSubtitle;
  settle: (state: SubtitleState) => void;
  rendererFailed: () => void;
}

/**
 * Draws the request's subtitle over its video with what `render` makes, until the cleanup it
 * returns, saying how it's getting on with `settle`, or calling `rendererFailed` if the renderer
 * won't start or throws.
 */
export function draw({ video, subtitle, settle, rendererFailed }: DrawRequest, render: Render) {
  let renderer: Renderer | undefined;
  let left = false;
  const failed = () => {
    if (!left) {
      rendererFailed();
    }
  };
  const ready = () => {
    settle("ready");
  };
  const adopt = (made: Renderer | null) => {
    if (made === null) {
      if (!left) {
        settle("failed");
      }
    } else if (left) {
      made.dispose();
    } else {
      renderer = made;
    }
  };
  const start = async () => {
    try {
      // Fetched afresh each time it's turned on, so what an earlier one came to no longer says.
      settle("loading");
      adopt(await render({ video, subtitle, ready, failed }));
    } catch {
      failed();
    }
  };
  void start();
  return () => {
    left = true;
    renderer?.dispose();
  };
}
