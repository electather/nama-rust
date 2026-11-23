import { PeopleLinks } from "@/features/title/components/people-links";
import { distinctPeople } from "@/features/title/lib/people";
import type { Role } from "@/gen/nama/v1/request_pb";

/** The first three of a title's Cast, as "Starring Tom Skerritt, Sigourney Weaver"; or nothing. */
export function TitleStarring({ cast }: { cast: Role[] }) {
  const people = distinctPeople(cast.slice(0, 3));
  if (people.length === 0) {
    return null;
  }
  return (
    <p className="text-sm text-foreground/80">
      Starring <PeopleLinks people={people} />
    </p>
  );
}
