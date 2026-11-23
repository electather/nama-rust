import { ContinueWatching } from "@/features/home/components/continue-watching";
import { FeaturedSection } from "@/features/home/components/featured-section";
import { HomeLayout } from "@/features/home/components/home-layout";
import { HomeRowSection } from "@/features/home/components/home-row-section";
import { LibraryShelf } from "@/features/home/components/library-shelf";
import { useHomeRows } from "@/features/home/data/use-home-rows";
import { KernelHomeRow } from "@/gen/nama/v1/browse_pb";

/**
 * Home: Featured, its Libraries, then its rows in the order the Server lists them, each loading on
 * its own.
 * Featured, Continue Watching and the first Plugin row load at once wherever they sit; each
 * later Plugin row loads as you scroll near it. A Kernel row this app doesn't know, from a newer
 * Server, shows nothing.
 */
export function HomePage() {
  const rows = useHomeRows();
  // Counted among Plugin rows only: the route's loader starts Continue Watching wherever it sits,
  // so counting it would load one more Plugin row at once whenever it moves down.
  const firstPluginRow = rows.findIndex(({ kernelOrPlugin }) => kernelOrPlugin.case === "plugin");
  return (
    <HomeLayout hero={<FeaturedSection />}>
      <LibraryShelf />
      {rows.map(({ kernelOrPlugin }, index) => {
        if (kernelOrPlugin.case === "plugin") {
          const row = kernelOrPlugin.value;
          return (
            <HomeRowSection
              key={`${row.pluginId}/${row.key}`}
              row={row}
              eager={index === firstPluginRow}
            />
          );
        }
        if (kernelOrPlugin.value === KernelHomeRow.CONTINUE_WATCHING) {
          return <ContinueWatching key="continue-watching" />;
        }
        return null;
      })}
    </HomeLayout>
  );
}
