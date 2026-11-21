import { QualitiesHead } from "@/features/admin-plugins/components/qualities-head";
import { QualitiesList } from "@/features/admin-plugins/components/qualities-list";
import { useQualities } from "@/features/admin-plugins/data/use-qualities";
import type { PluginDetails } from "@/shared/lib/plugins";

/** The Qualities heading over their list, or why they didn't load. */
export function QualitiesSection({ plugin }: { plugin: PluginDetails }) {
  const listed = useQualities(plugin.summary.id);
  const unloaded = (
    <>
      <QualitiesHead />
      <p role="alert" className="mt-4 text-sm text-destructive empty:hidden">
        {listed.error && "Couldn’t load the Qualities. Check that the Server is running."}
      </p>
    </>
  );
  return (
    <section aria-labelledby="qualities" className="mt-10">
      {listed.data ? <QualitiesList plugin={plugin} saved={listed.data.qualities} /> : unloaded}
    </section>
  );
}
