import { Link } from "@tanstack/react-router";

import { useTopBarLibraries } from "@/app/data/use-top-bar-libraries";
import { LibrariesMenu } from "@/app/top-bar/libraries-menu";
import { SearchButton } from "@/app/top-bar/search-button";
import { TopBarLinks } from "@/app/top-bar/top-bar-links";
import { UserMenu } from "@/app/top-bar/user-menu";
import { InboxMenu } from "@/features/inbox/components/inbox-menu";
import { Logo } from "@/shared/components/logo";

/**
 * The top bar's one row: the mark and the links, or on a phone the Libraries menu, at its start;
 * search, your notifications and your menu at its end. At the Hub, `atHub`, it links the Libraries
 * of every Server the page reached, and has no notifications.
 */
export function TopBarRow({
  userName,
  atHub = false,
  onSearch,
}: {
  userName: string;
  atHub?: boolean;
  onSearch: () => void;
}) {
  const groups = useTopBarLibraries(atHub);
  return (
    <div className="flex items-center gap-2.5 px-5 py-4.5 transition-all duration-300 ease-out-expo motion-reduce:transition-none raised:rounded-2xl raised:bg-card raised:px-3.5 raised:py-2">
      <Link
        to="/"
        className="shrink-0 rounded-sm outline-none focus-visible:ring-3 focus-visible:ring-ring/50"
      >
        <Logo size={32} label="Nama" />
      </Link>
      <span aria-hidden className="ms-3.75 hidden h-4 w-px shrink-0 bg-border sm:block" />
      <TopBarLinks groups={groups} />
      <LibrariesMenu groups={groups} />
      <span className="flex-1" />
      <SearchButton
        label={atHub ? "Search your Servers" : "Search this Server"}
        onSearch={onSearch}
      />
      {/* ponytail: none at the Hub until it merges each Server's Inbox. */}
      {!atHub && <InboxMenu />}
      <UserMenu userName={userName} atHub={atHub} />
    </div>
  );
}
