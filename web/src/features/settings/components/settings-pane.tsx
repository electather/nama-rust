import { useRef } from "react";

import { FoundSetting } from "@/features/settings/components/found-setting";
import { SectionBoundary } from "@/features/settings/components/section-boundary";
import { definitions } from "@/features/settings/lib/sections";
import type { User } from "@/gen/nama/v1/sign_in_pb";
import type { Section } from "@/shared/lib/settings-address";

/**
 * The pane of the `section` open: its title over its settings, which load on their own, landing on
 * the setting the address names. While they load it shows a skeleton, and in their place, if they
 * don't, what couldn't load and a way to try again; the rest of Settings stays as it is either way.
 */
export function SettingsPane({ section, user }: { section: Section; user: User | undefined }) {
  const { couldntLoad, pane: Pane } = definitions[section];
  const pane = useRef<HTMLDivElement>(null);
  return (
    // Under what sticks at the top of a pane, a field scrolled to for focus would be hidden.
    // ponytail: one height for whatever sticks, the tallest. Measure it if one outgrows this.
    <div
      ref={pane}
      className="flex min-w-0 flex-1 scrollbar-quiet flex-col overflow-y-auto overscroll-contain has-data-sticks:scroll-pt-48"
    >
      <SectionBoundary what={couldntLoad} rows={3} pane>
        <Pane user={user} />
        <FoundSetting pane={pane} section={section} />
      </SectionBoundary>
    </div>
  );
}
