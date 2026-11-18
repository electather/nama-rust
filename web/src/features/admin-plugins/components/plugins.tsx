import { PluginRow } from "@/features/admin-plugins/components/plugin-row";
import { usePluginSummaries } from "@/features/admin-plugins/data/use-plugin-summaries";
import { AdminList } from "@/shared/components/admin-list/admin-list";
import { Page } from "@/shared/components/page";

/** The Plugins page: every Plugin the Server knows, to enable, or to set up first. */
export function Plugins() {
  const summaries = usePluginSummaries();
  return (
    <Page heading="Plugins">
      <p className="mt-2 max-w-prose text-pretty text-muted-foreground">
        Plugins add to what the Server can do, such as finding Movies and Shows on TMDB.
      </p>
      <AdminList>
        {summaries.map((plugin) => (
          <PluginRow key={plugin.id} plugin={plugin} />
        ))}
      </AdminList>
    </Page>
  );
}
