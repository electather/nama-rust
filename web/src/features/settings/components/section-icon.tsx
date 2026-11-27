import {
  Bell,
  CalendarClock,
  CircleUser,
  Cpu,
  DatabaseBackup,
  FileQuestionMark,
  Film,
  HardDrive,
  History,
  House,
  KeyRound,
  Laptop,
  Library,
  type LucideIcon,
  MonitorPlay,
  Puzzle,
  Rows3,
  Server,
  Shield,
  SlidersHorizontal,
  Tv,
  Users,
} from "lucide-react";

import type { Section } from "@/shared/lib/settings-address";

const icons: Record<Section, LucideIcon> = {
  signin: Shield,
  devices: Tv,
  playback: SlidersHorizontal,
  home: House,
  requests: Film,
  connections: Bell,
  libraries: Library,
  "needs-a-match": FileQuestionMark,
  "now-playing": MonitorPlay,
  jobs: CalendarClock,
  server: HardDrive,
  users: Users,
  "default-home": Rows3,
  "server-playback": Cpu,
  backups: DatabaseBackup,
  plugins: Puzzle,
  account: CircleUser,
  "account-signin": KeyRound,
  "account-sessions": Laptop,
  "account-servers": Server,
  "account-history": History,
};

/** A section's icon, in the ink of the open one when it's `marked`. */
export function SectionIcon({ section, marked }: { section: Section; marked: boolean }) {
  const Icon = icons[section];
  return (
    <Icon aria-hidden strokeWidth={1.5} className={marked ? "size-4 text-primary" : "size-4"} />
  );
}
