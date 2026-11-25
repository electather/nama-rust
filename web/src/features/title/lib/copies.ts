import type { LinkedServer } from "@/gen/nama/hub/v1/link_pb";
import type { Version } from "@/gen/nama/v1/browse_pb";
import type { WatchState } from "@/gen/nama/v1/watch_state_pb";

/** A Movie, or a Show's Episode, as one of the Account's Servers has it, for Play to play there. */
export interface Copy {
  /** The Linked Server it's on, unset on a Server's own web app. */
  server: LinkedServer | undefined;
  libraryId: bigint;
  versions: Version[];
  watchState: WatchState | undefined;
}

/** Which Server Play plays from: one of `copies`, `copy` its choice, which `onChoose` changes. */
export interface PlayFrom {
  copies: Copy[];
  copy: Copy | undefined;
  onChoose: (serverId: string) => void;
}

/** Each Server's first of `copies`, as when a title is in two of its Libraries. */
export function onePerServer(copies: Copy[]) {
  return copies.filter(
    (copy, at) => copies.findIndex(({ server }) => server?.id === copy.server?.id) === at,
  );
}

/**
 * The copy on the Server `chosen` names, or else the one with the best Version, the largest file,
 * as Play picks a Version on one Server. On a tie, the first: the page's own.
 */
export function copyToPlay(copies: [Copy, ...Copy[]], chosen: string | undefined) {
  const picked = copies.find(({ server }) => server !== undefined && server.id === chosen);
  if (picked) {
    return picked;
  }
  let [best] = copies;
  for (const copy of copies) {
    if (largestFile(copy) > largestFile(best)) {
      best = copy;
    }
  }
  return best;
}

function largestFile({ versions }: Copy) {
  return Math.max(0, ...versions.map(({ size }) => Number(size)));
}
