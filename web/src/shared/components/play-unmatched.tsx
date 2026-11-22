import { Link } from "@tanstack/react-router";
import { Play } from "lucide-react";
import { buttonVariants } from "nama-ui/components/button";

/** Plays the Version `versionId` of the Library `libraryId`, which holds the Unmatched Item `name`. */
export function PlayUnmatched({
  libraryId,
  versionId,
  name,
}: {
  libraryId: bigint;
  versionId: bigint;
  name: string;
}) {
  return (
    <Link
      to="/libraries/$libraryId/unmatched/$versionId/play"
      params={{ libraryId: String(libraryId), versionId: String(versionId) }}
      aria-label={`Play ${name}`}
      className={buttonVariants({ variant: "ghost" })}
    >
      <Play aria-hidden />
      Play
    </Link>
  );
}
