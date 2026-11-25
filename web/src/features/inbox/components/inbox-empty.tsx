import { Bell } from "lucide-react";
import { Empty, EmptyHeader, EmptyMedia, EmptyTitle } from "nama-ui/components/empty";

/** What the bell's menu says before any notification has come. */
export function InboxEmpty() {
  const icon = (
    <EmptyMedia>
      <Bell aria-hidden strokeWidth={1.5} />
    </EmptyMedia>
  );
  return (
    <Empty>
      <EmptyHeader>
        {icon}
        <EmptyTitle>No notifications yet</EmptyTitle>
      </EmptyHeader>
    </Empty>
  );
}
