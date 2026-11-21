import { PersonalForm } from "@/features/settings/components/personal-form";
import type { PersonalSettings as Personal } from "@/gen/nama/v1/user_pb";

/** One Plugin's Personal settings, to change or forget. */
export function PluginCard({ plugin }: { plugin: Personal }) {
  const headingId = `personal-${plugin.pluginId}`;
  return (
    <li aria-labelledby={headingId} className="rounded-xl border bg-card p-4.5">
      <PersonalForm plugin={plugin} headingId={headingId} />
    </li>
  );
}
