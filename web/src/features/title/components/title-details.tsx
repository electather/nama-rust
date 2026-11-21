import { DetailList } from "@/features/title/components/detail-list";
import type { DetailGroup } from "@/features/title/lib/detail-group";

/** A title's Details at the foot of its page: each of `groups` with a fact, under a hairline. */
export function TitleDetails({ groups }: { groups: DetailGroup[] }) {
  const shown = groups
    .map(({ heading, facts }) => ({
      heading,
      facts: facts.filter(([, value]) => value !== undefined),
    }))
    .filter(({ facts }) => facts.length > 0);
  if (shown.length === 0) {
    return null;
  }
  return (
    <section aria-label="Details" className="border-t pt-7.5">
      <div className="grid gap-x-12 gap-y-7.5 sm:grid-cols-2 lg:grid-cols-3">
        {shown.map((group) => (
          <DetailList key={group.heading} {...group} />
        ))}
      </div>
    </section>
  );
}
