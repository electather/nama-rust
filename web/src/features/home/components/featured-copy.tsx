import { FeaturedActions } from "@/features/home/components/featured-actions";
import { FeaturedFacts } from "@/features/home/components/featured-facts";
import { TitleOrLogo } from "@/features/home/components/title-or-logo";
import type { Panel } from "@/features/home/lib/featured-panels";

/** An open panel's Title Logo: up to 112px tall, 144px from `sm`, as a title page shows it. */
const heroLogo = {
  className: "max-h-28 max-w-4/5 sm:max-h-36",
  sizes: "(max-width: 720px) 80vw, 448px",
};

/**
 * An open panel's copy over its art: its title, its facts, its overview and its actions, its Play
 * saying how far in you are. Hidden while closed, so only the open panel's is read or reached.
 */
export function FeaturedCopy({ panel }: { panel: Panel }) {
  return (
    <div className="pointer-events-none invisible absolute inset-s-5 bottom-5 flex w-featured-copy -translate-x-3.5 flex-col gap-2.5 opacity-0 transition-all duration-600 ease-out-expo group-data-open:visible group-data-open:translate-x-0 group-data-open:opacity-100 group-data-open:delay-120 motion-reduce:transition-none max-[720px]:inset-x-3.5 max-[720px]:bottom-3.5 max-[720px]:w-auto">
      <h1 className="text-hero font-medium text-balance">
        <TitleOrLogo title={panel.title} logoUrl={panel.logoUrl} logo={heroLogo} />
      </h1>
      <FeaturedFacts rating={panel.rating} facts={panel.facts} />
      {panel.overview !== undefined && (
        <p className="line-clamp-3 max-w-overview text-base/7 text-pretty">{panel.overview}</p>
      )}
      <FeaturedActions panel={panel} />
    </div>
  );
}
