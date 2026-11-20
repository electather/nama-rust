import { DoneNotice } from "@/features/home/components/done-notice";
import type { Done } from "@/features/home/hooks/use-card-actions";
import type { Hiding } from "@/features/home/lib/hiding";

/**
 * Says what a Continue Watching card's menu last did. It stays until you leave Home or act on
 * another, rather than timing out on someone still reading it.
 */
export function CardNotice({
  done,
  onUndo,
}: {
  done: Done | undefined;
  onUndo: (hiding: Hiding) => void;
}) {
  return (
    // Always there, so a screen reader announces what's put in it.
    <output className="block">{done && <DoneNotice done={done} onUndo={onUndo} />}</output>
  );
}
