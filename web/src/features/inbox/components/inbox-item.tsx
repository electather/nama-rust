import { Link } from "@tanstack/react-router";
import { cn } from "cn";
import { DropdownMenuLinkItem } from "nama-ui/components/dropdown-menu";

import { useMarkRead } from "@/features/inbox/data/use-mark-read";
import { notificationText } from "@/features/inbox/lib/notification-text";
import { sectionSearch } from "@/features/settings/lib/sections";
import type { InboxNotification } from "@/gen/nama/v1/inbox_pb";

/**
 * A notification, marked while unread, opening its Request among the User's Requests in Settings
 * and marking it read.
 */
export function InboxItem({ notification }: { notification: InboxNotification }) {
  const markRead = useMarkRead();
  const unread = notification.readAt === undefined;
  // ponytail: Settings' Requests has no address for one Request, so it opens the whole list.
  const link = <Link to="." search={(rest) => ({ ...rest, ...sectionSearch("requests") })} />;
  return (
    <DropdownMenuLinkItem
      render={link}
      onClick={() => {
        if (unread) {
          markRead.mutate({ id: notification.id });
        }
      }}
    >
      <span
        aria-hidden
        className={cn("size-1.5 shrink-0 rounded-full", unread && "bg-foreground")}
      />
      {unread && <span className="sr-only">Unread: </span>}
      <span className={cn("py-1.5", unread ? "font-medium" : "text-muted-foreground")}>
        {notificationText(notification)}
      </span>
    </DropdownMenuLinkItem>
  );
}
