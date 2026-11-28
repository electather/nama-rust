import type { LinkOptions } from "@tanstack/react-router";

import { listed } from "@/features/title/lib/listed";
import { similarMovies, similarShows } from "@/features/title/lib/similar-titles";
import type { PersonRole } from "@/gen/nama/v1/browse_pb";
import type { ExternalId } from "@/gen/nama/v1/match_pb";
import type { Role } from "@/gen/nama/v1/request_pb";
import { metadataLine } from "@/shared/lib/title-facts";
import { titleParam } from "@/shared/lib/title-param";

/** Where the page of the Person `personId` is. */
export function personLink(personId: ExternalId | undefined): LinkOptions {
  return { to: "/people/$personId", params: { personId: titleParam(personId) } };
}

/** The People of `roles`, each once at their first Role, in order; by name for one without an id. */
export function distinctPeople(roles: Role[]) {
  const seen = new Set<string>();
  return roles.filter((role) => {
    const person = role.personId ? titleParam(role.personId) : role.name;
    const first = !seen.has(person);
    seen.add(person);
    return first;
  });
}

/** The People of `crew` who did `job`, such as "Director", each once; or nothing. */
export function crewWithJob(crew: Role[], job: "Director" | "Creator") {
  const people = distinctPeople(crew.filter((role) => role.part === job));
  return people.length > 0 ? people : undefined;
}

/** The titles of a Person's `roles` as their page lists them, each labelled with its year and their parts. */
export function personTitles(roles: PersonRole[]) {
  return roles.flatMap(({ title, parts }) => {
    const detail = (year: string | undefined) => metadataLine(year, listed(parts));
    if (title.case === "movie") {
      return similarMovies([title.value], detail);
    }
    // A title this version can't show has none.
    return title.case === "show" ? similarShows([title.value], detail) : [];
  });
}
