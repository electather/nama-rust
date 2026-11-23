import { Link } from "@tanstack/react-router";
import { Fragment } from "react";

import { personLink } from "@/features/title/lib/people";
import type { Role } from "@/gen/nama/v1/request_pb";
import { textLink } from "@/shared/lib/text-link-classes";

/** The names of `people`, joined with commas, each opening its Person's page. */
export function PeopleLinks({ people }: { people: Role[] }) {
  return people.map((person, index) => (
    <Fragment key={`${person.personId?.namespace}-${person.personId?.value}`}>
      {index > 0 && ", "}
      <Link {...personLink(person.personId)} className={textLink}>
        {person.name}
      </Link>
    </Fragment>
  ));
}
