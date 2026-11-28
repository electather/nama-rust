import { Link } from "@tanstack/react-router";
import { Play } from "lucide-react";
import { buttonVariants } from "nama-ui/components/button";

/**
 * Plays the Version `versionId` of the Library `libraryId`, which holds the Unmatched Item `name`,
 * as a ghost button, or an `overlay` one over art.
 */
export function PlayUnmatched({
  libraryId,
  versionId,
  name,
  variant = "ghost",
}: {
  libraryId: bigint;
  versionId: bigint;
  name: string;
  variant?: "ghost" | "overlay";
}) {
  return (
    <Link
      to="/libraries/$libraryId/unmatched/$versionId/play"
      params={{ libraryId: String(libraryId), versionId: String(versionId) }}
      aria-label={`Play ${name}`}
      className={buttonVariants({ variant })}
    >
      <Play aria-hidden />
      Play
    </Link>
  );
}
