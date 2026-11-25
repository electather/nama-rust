import { DropdownMenuItem, DropdownMenuSeparator } from "nama-ui/components/dropdown-menu";

import { InboxEmpty } from "@/features/inbox/components/inbox-empty";
import { InboxItem } from "@/features/inbox/components/inbox-item";
import { useInboxNotifications } from "@/features/inbox/data/use-inbox-notifications";
import { useMarkAllRead } from "@/features/inbox/data/use-mark-all-read";

/** The signed-in User's notifications, newest first, then "Mark all read". */
export function InboxList() {
  const notifications = useInboxNotifications();
  const markAllRead = useMarkAllRead();
  if (notifications.length === 0) {
    return <InboxEmpty />;
  }
  const allRead = notifications.every(({ readAt }) => readAt !== undefined);
  return (
    <div className="w-80">
      {notifications.map((notification) => (
        <InboxItem key={String(notification.id)} notification={notification} />
      ))}
      <DropdownMenuSeparator />
      <DropdownMenuItem
        disabled={allRead || markAllRead.isPending}
        onClick={() => {
          markAllRead.mutate({});
        }}
      >
        Mark all read
      </DropdownMenuItem>
    </div>
  );
}
