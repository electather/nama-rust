import { CircleCheck } from "lucide-react";
import { Empty, EmptyHeader, EmptyMedia, EmptyTitle } from "nama-ui/components/empty";

/** Says everything is Matched, unless a Library listed above needs a Match (`anyListed`). */
export function EverythingMatched({ anyListed }: { anyListed: boolean }) {
  if (anyListed) {
    return null;
  }
  const icon = (
    <EmptyMedia>
      <CircleCheck aria-hidden strokeWidth={1.5} />
    </EmptyMedia>
  );
  return (
    <Empty>
      <EmptyHeader>
        {icon}
        <EmptyTitle>Everything is Matched</EmptyTitle>
      </EmptyHeader>
    </Empty>
  );
}
