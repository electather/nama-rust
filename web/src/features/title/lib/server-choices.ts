import type { LinkedServer } from "@/gen/nama/hub/v1/link_pb";
import type { Version } from "@/gen/nama/v1/browse_pb";
import type { WatchState } from "@/gen/nama/v1/watch_state_pb";

/** A Movie, or a Show's Episode, as one of the Account's Servers has it, for Play to play there. */
export interface ServerChoice {
  /** The Linked Server it's on, unset on a Server's own web app. */
  server: LinkedServer | undefined;
  libraryId: bigint;
  versions: Version[];
  watchState: WatchState | undefined;
}

/** A title as the page's own Server holds it, which names no Linked Server. */
export type OwnChoice = Omit<ServerChoice, "server">;

/** Which Server Play plays from: `choice`, one of `choices`, which `onChoose` changes. */
export interface PlayFrom {
  choices: ServerChoice[];
  choice: ServerChoice | undefined;
  onChoose: (serverId: string) => void;
}

/** Each Server's first of `choices`, as when a title is in two of its Libraries. */
export function onePerServer(choices: ServerChoice[]) {
  return choices.filter(
    (choice, at) => choices.findIndex(({ server }) => server?.id === choice.server?.id) === at,
  );
}

/**
 * The choice on the Server `chosen` names, or else the one with the best Version, the largest file,
 * as Play picks a Version on one Server. On a tie, the first: the page's own.
 */
export function choiceToPlay(
  choices: [ServerChoice, ...ServerChoice[]],
  chosen: string | undefined,
) {
  const picked = choices.find(({ server }) => server !== undefined && server.id === chosen);
  if (picked) {
    return picked;
  }
  let [best] = choices;
  for (const choice of choices) {
    if (largestFile(choice) > largestFile(best)) {
      best = choice;
    }
  }
  return best;
}

function largestFile({ versions }: ServerChoice) {
  return Math.max(0, ...versions.map(({ size }) => Number(size)));
}
