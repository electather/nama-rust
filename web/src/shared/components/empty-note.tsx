import type { LucideIcon } from "lucide-react";
import {
  Empty,
  EmptyDescription,
  EmptyHeader,
  EmptyMedia,
  EmptyTitle,
} from "nama-ui/components/empty";

/** Says there's nothing to show here: its `Icon`, its `title`, and what to do, if anything (`todo`). */
export function EmptyNote({
  Icon,
  title,
  todo,
}: {
  Icon: LucideIcon;
  title: string;
  todo?: string;
}) {
  const icon = (
    <EmptyMedia>
      <Icon aria-hidden strokeWidth={1.5} />
    </EmptyMedia>
  );
  return (
    <Empty>
      <EmptyHeader>
        {icon}
        <EmptyTitle>{title}</EmptyTitle>
        {todo !== undefined && <EmptyDescription>{todo}</EmptyDescription>}
      </EmptyHeader>
    </Empty>
  );
}
