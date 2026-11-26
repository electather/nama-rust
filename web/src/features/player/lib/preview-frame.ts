import type { Transport } from "@connectrpc/connect";

import type { PlaybackPlan, PreviewFrames } from "@/gen/nama/v1/playback_pb";
import { onServer } from "@/shared/lib/server";

/** Where a Version's Preview frames are: their layout, and the path its sheets are numbered under. */
export interface FrameSheets {
  frames: PreviewFrames;
  /** Such as `/media/signed/<grant>/<signature>/versions/7/preview-frames` on the Server, which `/<n>.jpg` follows. */
  path: string;
}

/**
 * The Preview frames of Version `versionId` on the Server `transport` talks to, as `plan` lays
 * them out, if it has any. They load under what precedes `/versions/` or `/plans/` in the plan's URL, which
 * is its signed prefix when signed (an `<img>` can't send a bearer token, ADR 0105), else `/media`.
 */
export function frameSheets(plan: PlaybackPlan, versionId: bigint, transport: Transport) {
  const { previewFrames: frames } = plan;
  const prefix = /^(?<prefix>.*?)\/(?:versions|plans)\//u.exec(plan.url)?.groups?.["prefix"];
  return frames && frames.count > 0
    ? {
        frames,
        path: onServer(transport, `${prefix ?? "/media"}/versions/${versionId}/preview-frames`),
      }
    : undefined;
}

/**
 * The frame nearest `seconds`: the `src` of its sheet, and where in it the frame is, in pixels.
 * Every sheet is laid out as a full one, the last padded where it's short.
 */
export function frameAt({ frames, path }: FrameSheets, seconds: number): FrameCrop {
  const { intervalMs, width, height, columns, rows, count } = frames;
  const index = Math.min(count - 1, Math.max(0, Math.round((seconds * 1000) / intervalMs)));
  const perSheet = columns * rows;
  const cell = index % perSheet;
  return {
    src: `${path}/${Math.floor(index / perSheet)}.jpg`,
    x: (cell % columns) * width,
    y: Math.floor(cell / columns) * height,
    width,
    height,
  };
}

/** One Preview frame: its sheet's `src`, and the frame's box within it, in pixels. */
export interface FrameCrop {
  src: string;
  x: number;
  y: number;
  width: number;
  height: number;
}
