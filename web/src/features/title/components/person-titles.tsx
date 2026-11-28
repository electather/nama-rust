import { useId } from "react";

import { personTitles } from "@/features/title/lib/people";
import type { PersonRole } from "@/gen/nama/v1/browse_pb";
import { Poster } from "@/shared/components/poster/poster";
import { posterGrid } from "@/shared/lib/tile-shapes";

/** A Person's titles on this Server under `heading`, as a Library's posters, each with their part. */
export function PersonTitles({ heading, roles }: { heading: string; roles: PersonRole[] }) {
  const headingId = useId();
  if (roles.length === 0) {
    return null;
  }
  const items = personTitles(roles).map(({ key, ...title }) => (
    <li key={key}>
      <Poster {...title} size="tile" />
    </li>
  ));
  return (
    <section className="mt-10">
      <h2 id={headingId} className="text-lg font-semibold">
        {heading}
      </h2>
      <ul aria-labelledby={headingId} className={posterGrid}>
        {items}
      </ul>
    </section>
  );
}
