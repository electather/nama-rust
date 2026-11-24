import { useTransport } from "@connectrpc/connect-query";

import { frameSheets } from "@/features/player/lib/preview-frame";
import type { PreviewFrames } from "@/gen/nama/v1/playback_pb";

/** `frameSheets` on the Server the player plays from. */
export function useFrameSheets(frames: PreviewFrames | undefined, versionId: bigint) {
  return frameSheets(frames, versionId, useTransport());
}
