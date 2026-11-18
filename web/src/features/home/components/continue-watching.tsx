import { ContinueWatchingItem } from "@/features/home/components/continue-watching-item";
import { HiddenNotice } from "@/features/home/components/hidden-notice";
import { useContinueWatching } from "@/features/home/data/use-continue-watching";
import { useHideFromContinueWatching } from "@/features/home/hooks/use-hide-from-continue-watching";
import { entryCard } from "@/features/home/lib/continue-watching-cards";
import { BackdropGrid } from "@/shared/components/backdrop-grid";

/**
 * What you've started and not played, and the next Episode of each Show you're watching, most
 * recent first, each with a menu to hide it. Nothing at all when there's none and nothing to undo.
 */
export function ContinueWatching() {
  const entries = useContinueWatching();
  const { lastHidden, hide, undo } = useHideFromContinueWatching();
  const cards = entries.flatMap((entry) => entryCard(entry));
  if (cards.length === 0 && !lastHidden) {
    return null;
  }
  return (
    <section aria-labelledby="continue-watching" className="mt-10">
      <h2 id="continue-watching" className="font-medium">
        Continue Watching
      </h2>
      <HiddenNotice lastHidden={lastHidden} onUndo={undo} />
      {cards.length > 0 && (
        <BackdropGrid>
          {cards.map((card) => (
            <ContinueWatchingItem
              key={card.hiding.key}
              card={card}
              onHide={() => {
                hide(card.hiding);
              }}
            />
          ))}
        </BackdropGrid>
      )}
    </section>
  );
}
