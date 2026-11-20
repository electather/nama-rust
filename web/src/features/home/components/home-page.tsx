import { ContinueWatching } from "@/features/home/components/continue-watching";
import { FeaturedSection } from "@/features/home/components/featured-section";
import { HomeLayout } from "@/features/home/components/home-layout";
import { HomeRowSection } from "@/features/home/components/home-row-section";
import { useHomeRows } from "@/features/home/data/use-home-rows";

/**
 * Home: Featured, Continue Watching, then the rows Plugins give, each loading on its own. The
 * top three load at once, in that order; each later row loads as you scroll near it.
 */
export function HomePage() {
  const rows = useHomeRows();
  return (
    <HomeLayout hero={<FeaturedSection />}>
      <ContinueWatching />
      {rows.map((row, index) => (
        <HomeRowSection key={`${row.pluginId}/${row.key}`} row={row} eager={index === 0} />
      ))}
    </HomeLayout>
  );
}
