import { Link } from "@tanstack/react-router";
import {
  Bell,
  Cpu,
  DatabaseBackup,
  Film,
  HardDrive,
  House,
  type LucideIcon,
  Puzzle,
  Rows3,
  Shield,
  SlidersHorizontal,
  Tv,
  Users,
} from "lucide-react";
import { useEffect, useRef } from "react";

import { type Section, sectionSearch, sectionTitles } from "@/features/settings/lib/sections";

const icons: Record<Section, LucideIcon> = {
  signin: Shield,
  devices: Tv,
  playback: SlidersHorizontal,
  home: House,
  requests: Film,
  connections: Bell,
  server: HardDrive,
  users: Users,
  "default-home": Rows3,
  "server-playback": Cpu,
  backups: DatabaseBackup,
  plugins: Puzzle,
};

const LINK =
  "flex h-11 flex-none items-center gap-2.5 rounded-sm px-2 text-sm whitespace-nowrap transition-colors outline-none focus-visible:ring-3 focus-visible:ring-ring/50 motion-reduce:transition-none md:h-7";
// The open section is filled and bold, so it isn't marked by colour alone.
const CURRENT = `${LINK} bg-popover font-medium text-foreground`;
const OTHER = `${LINK} text-muted-foreground hover:bg-muted hover:text-foreground`;

/** A section's link in the sidebar, marked while it's the one open. */
export function SectionLink({ section, current }: { section: Section; current: boolean }) {
  const link = useRef<HTMLAnchorElement>(null);
  useEffect(() => {
    // On a phone the open section may sit outside the strip, as when a link opened Settings on it.
    if (current) {
      link.current?.scrollIntoView({ block: "nearest", inline: "nearest" });
    }
  }, [current]);
  const Icon = icons[section];
  return (
    <Link
      ref={link}
      to="."
      search={(rest) => ({ ...rest, ...sectionSearch(section) })}
      // Sections share one history entry, so Back closes Settings.
      replace
      aria-current={current ? "page" : undefined}
      className={current ? CURRENT : OTHER}
    >
      <Icon
        aria-hidden
        strokeWidth={1.5}
        className={current ? "size-4 text-primary max-md:hidden" : "size-4 max-md:hidden"}
      />
      {sectionTitles[section]}
    </Link>
  );
}
