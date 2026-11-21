import { UserRound } from "lucide-react";

import type { Role } from "@/gen/nama/v1/request_pb";
import { useSrcSet } from "@/shared/hooks/use-src-set";

/** One Role in a title's Cast & Crew: the Person's headshot, or a plain circle, over who and what. */
export function RoleCard({ role }: { role: Role }) {
  const srcSet = useSrcSet(role.headshotUrl);
  return (
    <li className="flex w-24 snap-start flex-col items-center gap-2 text-center text-sm">
      {role.headshotUrl === undefined ? (
        <span className="flex size-24 items-center justify-center rounded-full bg-muted text-muted-foreground">
          <UserRound aria-hidden className="size-10" />
        </span>
      ) : (
        // Its name is under it, so the image itself says nothing more.
        <img
          src={role.headshotUrl}
          srcSet={srcSet}
          sizes="96px"
          alt=""
          loading="lazy"
          className="size-24 rounded-full bg-muted object-cover outline -outline-offset-1 outline-input"
        />
      )}
      <span className="line-clamp-2 font-medium">{role.name}</span>
      {role.part !== "" && <span className="line-clamp-2 text-muted-foreground">{role.part}</span>}
    </li>
  );
}
