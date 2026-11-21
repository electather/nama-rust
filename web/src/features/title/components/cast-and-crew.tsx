import { RoleRow } from "@/features/title/components/role-row";
import type { Role } from "@/gen/nama/v1/request_pb";

/** A title's Cast, then its Crew, under a heading; nothing if it has neither. */
export function CastAndCrew({ cast, crew }: { cast: Role[]; crew: Role[] }) {
  const roles = [...cast, ...crew];
  if (roles.length === 0) {
    return null;
  }
  return (
    <section aria-label="Cast & Crew" className="flex flex-col gap-4">
      <h2 className="text-lg font-semibold">Cast & Crew</h2>
      <RoleRow roles={roles} />
    </section>
  );
}
