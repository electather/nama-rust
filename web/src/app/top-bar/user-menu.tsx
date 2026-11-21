import { useNavigate } from "@tanstack/react-router";
import { ChevronDown } from "lucide-react";

import { useSignOut } from "@/app/data/use-sign-out";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/shared/ui/dropdown-menu";

/** The signed-in User's menu, behind their initial: their Settings, and signing out. */
export function UserMenu({ userName }: { userName: string }) {
  const navigate = useNavigate();
  const signOut = useSignOut();
  return (
    <DropdownMenu>
      <DropdownMenuTrigger
        render={
          <button
            type="button"
            aria-label={`${userName}’s menu`}
            className="inline-flex h-9 shrink-0 items-center gap-1 rounded-full border border-input bg-card ps-0.75 pe-2 text-foreground outline-none focus-visible:border-ring focus-visible:ring-3 focus-visible:ring-ring/50"
          />
        }
      >
        <span className="grid size-7 place-items-center rounded-full bg-muted text-xs font-semibold">
          {userName.slice(0, 1).toUpperCase()}
        </span>
        <ChevronDown aria-hidden strokeWidth={1.5} className="size-3.5" />
      </DropdownMenuTrigger>
      <DropdownMenuContent>
        <DropdownMenuItem
          onClick={() => {
            // A history entry of its own, so Back closes Settings.
            void navigate({ to: ".", search: (rest) => ({ ...rest, settings: "playback" }) });
          }}
        >
          Settings
        </DropdownMenuItem>
        <DropdownMenuItem
          disabled={signOut.isPending}
          onClick={() => {
            signOut.mutate({});
          }}
        >
          Sign out
        </DropdownMenuItem>
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
