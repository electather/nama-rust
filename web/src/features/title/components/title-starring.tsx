import { listed } from "@/features/title/lib/listed";
import type { Role } from "@/gen/nama/v1/request_pb";

/** The first three of a title's Cast, as "Starring Tom Skerritt, Sigourney Weaver"; or nothing. */
export function TitleStarring({ cast }: { cast: Role[] }) {
  const names = listed(cast.slice(0, 3).map((role) => role.name));
  if (names === undefined) {
    return null;
  }
  return <p className="text-sm text-foreground/80">Starring {names}</p>;
}
