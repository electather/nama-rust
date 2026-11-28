import { CircleCheck } from "lucide-react";

import { EmptyNote } from "@/shared/components/empty-note";

/** Says everything is Matched, unless a Library listed above needs a Match (`anyListed`). */
export function EverythingMatched({ anyListed }: { anyListed: boolean }) {
  return anyListed ? null : <EmptyNote Icon={CircleCheck} title="Everything is Matched" />;
}
