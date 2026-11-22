import { TabsList, TabsTab } from "nama-ui/components/tabs";

/** The tabs over a Movies Library's titles: its Movies, then its Box Sets. */
export function LibraryTabList() {
  return (
    <div className="mt-4">
      <TabsList aria-label="View">
        <TabsTab value="movies">Movies</TabsTab>
        <TabsTab value="box-sets">Box Sets</TabsTab>
      </TabsList>
    </div>
  );
}
