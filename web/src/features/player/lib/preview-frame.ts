import type { PreviewFrames } from "@/gen/nama/v1/playback_pb";

/** Where a Version's Preview frames are: their layout, and the path its sheets are numbered under. */
export interface FrameSheets {
  frames: PreviewFrames;
  /** Such as `/media/versions/7/preview-frames`, which `/<n>.jpg` follows. */
  path: string;
}

/** The Preview frames of Version `versionId`, as its plan lays them out, if it has any. */
export function frameSheets(frames: PreviewFrames | undefined, versionId: bigint) {
  return frames && frames.count > 0
    ? { frames, path: `/media/versions/${versionId}/preview-frames` }
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
