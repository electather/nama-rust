import { Link } from "@tanstack/react-router";

import { Headshot } from "@/features/title/components/headshot";
import { personLink } from "@/features/title/lib/people";
import type { Role } from "@/gen/nama/v1/request_pb";

/** One Role in a title's Cast & Crew: the Person's headshot over who and what, opening their page. */
export function RoleCard({ role }: { role: Role }) {
  return (
    <li className="w-32 snap-start">
      <Link
        {...personLink(role.personId)}
        className="flex flex-col items-center gap-2 rounded-xl text-center text-sm outline-none focus-visible:ring-3 focus-visible:ring-ring/50"
      >
        <Headshot url={role.headshotUrl} />
        <span className="line-clamp-2 font-medium">{role.name}</span>
        {role.part !== "" && (
          <span className="line-clamp-2 text-muted-foreground">{role.part}</span>
        )}
      </Link>
    </li>
  );
}
