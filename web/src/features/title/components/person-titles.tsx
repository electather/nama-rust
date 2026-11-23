import { useId } from "react";

import { PersonTitleCard } from "@/features/title/components/person-title-card";
import { personTitles } from "@/features/title/lib/people";
import type { PersonRole } from "@/gen/nama/v1/browse_pb";
import { posterGrid } from "@/shared/lib/poster-classes";

/** A Person's titles on this Server under `heading`, as a Library's posters, each with their part. */
export function PersonTitles({ heading, roles }: { heading: string; roles: PersonRole[] }) {
  const headingId = useId();
  if (roles.length === 0) {
    return null;
  }
  return (
    <section className="mt-10">
      <h2 id={headingId} className="text-lg font-semibold">
        {heading}
      </h2>
      <ul aria-labelledby={headingId} className={posterGrid}>
        {personTitles(roles).map(({ title, parts }) => (
          <PersonTitleCard key={title.key} title={title} parts={parts} />
        ))}
      </ul>
    </section>
  );
}
