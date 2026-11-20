import { ContinueWatching } from "@/features/home/components/continue-watching";
import { FeaturedSection } from "@/features/home/components/featured-section";
import { HomeLayout } from "@/features/home/components/home-layout";
import { HomeRowSection } from "@/features/home/components/home-row-section";
import { useHomeRows } from "@/features/home/data/use-home-rows";
import { KernelHomeRow } from "@/gen/nama/v1/browse_pb";

/**
 * Home: Featured, then its rows in the order the Server lists them, each loading on its own.
 * Featured and the top two rows load at once; each later row loads as you scroll near it. A
 * Kernel row this app doesn't know, from a newer Server, shows nothing.
 */
export function HomePage() {
  const rows = useHomeRows();
  return (
    <HomeLayout hero={<FeaturedSection />}>
      {rows.map(({ kernelOrPlugin }, index) => {
        if (kernelOrPlugin.case === "plugin") {
          const row = kernelOrPlugin.value;
          return <HomeRowSection key={`${row.pluginId}/${row.key}`} row={row} eager={index < 2} />;
        }
        if (kernelOrPlugin.value === KernelHomeRow.CONTINUE_WATCHING) {
          return <ContinueWatching key="continue-watching" />;
        }
        return null;
      })}
    </HomeLayout>
  );
}
