import { RemovedNotice } from "@/features/home/components/removed-notice";
import { WatchedNotice } from "@/features/home/components/watched-notice";
import type { Done } from "@/features/home/hooks/use-card-actions";
import type { Hiding } from "@/features/home/lib/hiding";

/** What a Continue Watching card's menu `done`: removed, with Undo, or marked as watched. */
export function DoneNotice({ done, onUndo }: { done: Done; onUndo: (hiding: Hiding) => void }) {
  if (done.kind === "removed") {
    return <RemovedNotice key={done.hiding.key} hiding={done.hiding} onUndo={onUndo} />;
  }
  return <WatchedNotice key={done.marking.key} marking={done.marking} />;
}
