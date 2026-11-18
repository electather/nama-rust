import { Link } from "@tanstack/react-router";
import { LogOut, Settings } from "lucide-react";

import { useSignOut } from "@/app/data/use-sign-out";
import { DropdownMenuContent, DropdownMenuItem } from "@/shared/ui/dropdown-menu";

const settings = <Link to="/settings" />;

/** The User's menu: Settings, and signing out of this Device. */
export function UserMenuItems() {
  const signOut = useSignOut();
  return (
    <DropdownMenuContent>
      <DropdownMenuItem render={settings}>
        <Settings aria-hidden strokeWidth={1.5} className="size-4" />
        Settings
      </DropdownMenuItem>
      <DropdownMenuItem
        disabled={signOut.isPending}
        onClick={() => {
          signOut.mutate({});
        }}
      >
        <LogOut aria-hidden strokeWidth={1.5} className="size-4" />
        Sign out
      </DropdownMenuItem>
    </DropdownMenuContent>
  );
}
