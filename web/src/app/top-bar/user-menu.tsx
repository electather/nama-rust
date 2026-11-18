import { ChevronDown } from "lucide-react";
import { Suspense, lazy, useState } from "react";

import { DropdownMenu, DropdownMenuTrigger } from "@/shared/ui/dropdown-menu";

const UserMenuItems = lazy(async () => {
  const { UserMenuItems: items } = await import("@/app/top-bar/user-menu-items");
  return { default: items };
});

const trigger = (
  <button
    type="button"
    aria-label="Your menu"
    className="inline-flex h-9 shrink-0 cursor-pointer items-center gap-1 rounded-full border border-input bg-card ps-0.75 pe-2 text-foreground-soft transition-colors duration-150 ease-out-expo outline-none hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 focus-visible:ring-offset-background motion-reduce:transition-none"
  />
);

/** The signed-in User's button, showing their initials and opening Settings and Sign out. */
export function UserMenu({ userName }: { userName: string }) {
  const [opened, setOpened] = useState(false);
  return (
    <DropdownMenu
      onOpenChange={(open) => {
        setOpened(opened || open);
      }}
    >
      <DropdownMenuTrigger render={trigger}>
        <span
          aria-hidden
          className="grid size-7 place-items-center rounded-full bg-muted text-xs font-semibold text-foreground"
        >
          {initialsOf(userName)}
        </span>
        <ChevronDown aria-hidden strokeWidth={1.5} className="size-3.5" />
      </DropdownMenuTrigger>
      {opened && (
        <Suspense fallback={null}>
          <UserMenuItems />
        </Suspense>
      )}
    </DropdownMenu>
  );
}

/** The first letters of the first two words of `name`, upper-cased, such as "SK" for "Sam Kay". */
function initialsOf(name: string) {
  return name
    .split(/\s+/u)
    .filter((word) => word !== "")
    .slice(0, 2)
    .map((word) => String.fromCodePoint(word.codePointAt(0) ?? 0))
    .join("")
    .toUpperCase();
}
