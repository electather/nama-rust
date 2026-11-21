import type { RefObject } from "react";

import { CantDeleteItem } from "@/features/title/components/cant-delete-item";
import { OpenerItem } from "@/features/title/components/opener-item";
import type { Opener } from "@/shared/lib/opener";

/** Delete, which `remove` opens, or, while `readOnlyFolder` holds the Movie, why it can't be. */
export function DeleteMovieItem({
  remove,
  readOnlyFolder,
}: {
  remove: RefObject<Opener | null>;
  readOnlyFolder: string | undefined;
}) {
  return readOnlyFolder === undefined ? (
    <OpenerItem opener={remove}>Delete</OpenerItem>
  ) : (
    <CantDeleteItem folder={readOnlyFolder} />
  );
}
