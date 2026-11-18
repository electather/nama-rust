import type { ErrorComponentProps } from "@tanstack/react-router";

import { DeletedWhilePlaying } from "@/app/route-error/deleted-while-playing";
import { RouteError } from "@/app/route-error/route-error";
import { RefusalReason } from "@/gen/nama/v1/refusal_pb";
import { refusalOf } from "@/shared/lib/forms";

/** What the player's page shows instead when it fails: a Version gone while it played was deleted. */
export function PlayerError(props: ErrorComponentProps) {
  if (refusalOf(props.error).reason === RefusalReason.VERSION_NOT_FOUND) {
    return <DeletedWhilePlaying />;
  }
  return <RouteError {...props} />;
}
