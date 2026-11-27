import { SectionLink } from "@/features/settings/components/section-link";
import type { SectionGroup } from "@/features/settings/lib/sections";
import type { Section } from "@/shared/lib/settings-address";

/**
 * The sections of Settings as links under their groups' headings: down the side, the one open
 * marked, or on a phone the rows of the first page, each opening its section's page.
 */
export function SectionNav({ groups, section }: { groups: SectionGroup[]; section: Section }) {
  return (
    <nav aria-label="Settings" className="flex min-w-0 flex-col gap-0.5">
      {groups.map((group) => (
        <div key={group.label} className="flex flex-col gap-0.5">
          <h3 className="px-2 pt-2.5 pb-1 text-xs text-muted-foreground max-md:pt-4">
            {group.label}
          </h3>
          {group.items.map((item) => (
            <SectionLink key={item} section={item} current={item === section} />
          ))}
        </div>
      ))}
    </nav>
  );
}
