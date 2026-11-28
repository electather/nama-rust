import { Link } from "@tanstack/react-router";
import { Play } from "lucide-react";
import { buttonVariants } from "nama-ui/components/button";

/**
 * Plays the Version `versionId` of the Library `libraryId`, which holds the Unmatched Item `name`,
 * as a ghost button, or an `overlay` one over art, or a `secondary` one a phone's finger reaches
 * as `label` says, "Play" unless it says otherwise.
 */
export function PlayUnmatched({
  libraryId,
  versionId,
  name,
  variant = "ghost",
  label = "Play",
}: {
  libraryId: bigint;
  versionId: bigint;
  name: string;
  variant?: "ghost" | "overlay" | "secondary";
  label?: string;
}) {
  return (
    <Link
      to="/libraries/$libraryId/unmatched/$versionId/play"
      params={{ libraryId: String(libraryId), versionId: String(versionId) }}
      aria-label={`${label} ${name}`}
      className={buttonVariants({ variant, size: variant === "secondary" ? "touch" : "default" })}
    >
      <Play aria-hidden />
      {label}
    </Link>
  );
}
