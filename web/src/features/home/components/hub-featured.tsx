import { FeaturedAccordion } from "@/features/home/components/featured-accordion";
import { useHubFeatured } from "@/features/home/data/use-hub-featured";

/** Featured at the Hub, from the Account's Servers as each answers, or nothing till one gives any. */
export function HubFeatured() {
  return <FeaturedAccordion panels={useHubFeatured()} />;
}
