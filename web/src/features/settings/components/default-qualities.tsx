import { DefaultQualityForm } from "@/features/settings/components/default-quality-form";
import { useDefaultQualities } from "@/features/settings/data/use-default-qualities";

/**
 * The signed-in User's own default Quality for each Fulfiller with more than one, which their
 * Requests get when they don't pick. For a User who may choose a Quality; nothing while no
 * Fulfiller has anything to pick or flag.
 */
export function DefaultQualities() {
  const { data } = useDefaultQualities();
  if (!data || data.defaults.length === 0) {
    return null;
  }
  return (
    <section aria-labelledby="default-quality" className="mt-12 max-w-sm">
      <h2 id="default-quality" className="font-medium">
        Your default Quality
      </h2>
      <p className="mt-1 text-sm text-muted-foreground">
        What your Requests are downloaded at when you don’t pick a Quality.
      </p>
      {data.defaults.map((fulfiller) => (
        <DefaultQualityForm key={fulfiller.pluginId} fulfiller={fulfiller} />
      ))}
    </section>
  );
}
