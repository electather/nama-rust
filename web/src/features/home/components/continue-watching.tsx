import { ContinueWatchingItem } from "@/features/home/components/continue-watching-item";
import { useContinueWatching } from "@/features/home/data/use-continue-watching";
import { entryCard } from "@/features/home/lib/continue-watching-cards";
import { BackdropGrid } from "@/shared/components/backdrop-grid";

/**
 * What you've started and not played, and the next Episode of each Show you're watching, most
 * recent first, each with a menu to mark it watched. Nothing at all when there's none.
 */
export function ContinueWatching() {
  const entries = useContinueWatching();
  const cards = entries.flatMap((entry) => entryCard(entry));
  if (cards.length === 0) {
    return null;
  }
  return (
    <section aria-labelledby="continue-watching" className="mt-10">
      <h2 id="continue-watching" className="font-medium">
        Continue Watching
      </h2>
      <BackdropGrid>
        {cards.map((card) => (
          <ContinueWatchingItem key={card.key} card={card} />
        ))}
      </BackdropGrid>
    </section>
  );
}
