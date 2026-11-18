import { CardHeading } from "@/features/settings/components/card-heading";
import { PersonalForm } from "@/features/settings/components/personal-form";
import type { PersonalSettings as Personal } from "@/gen/nama/v1/user_pb";

/** One Plugin's Personal settings to save or forget, as listed at `listedAt`. */
export function PluginCard({ plugin, listedAt }: { plugin: Personal; listedAt: number }) {
  const headingId = `personal-${plugin.pluginId}`;
  return (
    <li aria-labelledby={headingId} className="rounded-lg border bg-card p-4.5">
      <CardHeading plugin={plugin} id={headingId} />
      <PersonalForm key={listedAt} plugin={plugin} />
    </li>
  );
}
