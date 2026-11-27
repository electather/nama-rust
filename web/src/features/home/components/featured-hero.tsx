import { FeaturedAccordion } from "@/features/home/components/featured-accordion";
import { useFeatured } from "@/features/home/data/use-featured";

/** Your Featured on this Server, or nothing when there's none. */
export function FeaturedHero() {
  return <FeaturedAccordion panels={useFeatured()} />;
}
