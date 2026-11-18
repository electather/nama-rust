import { HomeRowSection } from "@/features/home/components/home-row-section";
import { useHomeRows } from "@/features/home/data/use-home-rows";

/**
 * The Home rows Plugins give, after the Kernel's, each loading on its own. Home never waits on
 * them, and shows nothing for one that fails: a Plugin's failures belong on the Plugins page.
 */
export function HomeRows() {
  return useHomeRows()?.map((row) => (
    <HomeRowSection key={`${row.pluginId}/${row.key}`} row={row} />
  ));
}
