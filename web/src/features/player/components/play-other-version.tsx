import { Link } from "@tanstack/react-router";
import { Play } from "lucide-react";

import { buttonVariants } from "@/shared/ui/button";

/** Plays Version `id` in this player instead, named for its `picture` size, such as "1080p". */
export function PlayOtherVersion({ id, picture }: { id: bigint; picture: string | undefined }) {
  return (
    <Link
      to="."
      search={(before) => ({ ...before, version: Number(id) })}
      replace
      className={buttonVariants()}
    >
      <Play aria-hidden />
      {picture === undefined ? "Play other Version" : `Play ${picture} Version`}
    </Link>
  );
}
