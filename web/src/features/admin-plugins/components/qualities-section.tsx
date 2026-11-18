import { QualitiesForm } from "@/features/admin-plugins/components/qualities-form";
import { useQualities } from "@/features/admin-plugins/data/use-qualities";
import { type PluginDetails, pluginName } from "@/lib/plugins";

/** The Qualities heading and its form, or why they didn't load. */
export function QualitiesSection({ plugin }: { plugin: PluginDetails }) {
  const listed = useQualities(plugin.summary.id);
  return (
    <section aria-labelledby="qualities" className="mt-10">
      <h2 id="qualities" className="font-medium">
        Qualities
      </h2>
      <p className="mt-1 max-w-prose text-sm text-pretty text-muted-foreground">
        What Requests are downloaded at: {pluginName(plugin.summary)}’s quality profiles under names
        you choose. A new Request gets the default. With none, the Quality profile setting applies.
      </p>
      {listed.data ? (
        <QualitiesForm plugin={plugin} saved={listed.data.qualities} />
      ) : (
        <p role="alert" className="mt-4 text-sm text-destructive empty:hidden">
          {listed.error && "Couldn’t load the Qualities. Check that the Server is running."}
        </p>
      )}
    </section>
  );
}
