import { Bell } from "lucide-react";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuTrigger,
} from "nama-ui/components/dropdown-menu";

import { InboxLoader } from "@/features/inbox/components/inbox-loader";
import { useUnreadCount } from "@/features/inbox/data/use-unread-count";

/** The top bar's bell, badged with how many notifications are unread, opening a menu of them. */
export function InboxMenu() {
  const unread = useUnreadCount();
  const label = unread === 0 ? "Notifications" : `Notifications, ${unread} unread`;
  return (
    <DropdownMenu>
      <DropdownMenuTrigger
        render={
          <button
            type="button"
            aria-label={label}
            className="relative inline-flex size-9 shrink-0 items-center justify-center rounded-lg border border-input bg-input/30 text-muted-foreground transition-colors outline-none hover:text-foreground focus-visible:border-ring focus-visible:ring-3 focus-visible:ring-ring/50"
          />
        }
      >
        <Bell aria-hidden strokeWidth={1.5} className="size-4" />
        {unread > 0 && (
          <span
            aria-hidden
            className="absolute -inset-e-1.5 -top-1.5 grid h-4.5 min-w-4.5 place-items-center rounded-full bg-foreground px-1 text-xs font-medium text-background tabular-nums"
          >
            {unread > 99 ? "99+" : unread}
          </span>
        )}
      </DropdownMenuTrigger>
      <DropdownMenuContent>
        <InboxLoader />
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
