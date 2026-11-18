import {
  Film,
  House,
  Inbox,
  Keyboard,
  type LucideProps,
  RefreshCw,
  Settings,
  ShieldCheck,
  Tv,
} from "lucide-react";
import type { ComponentType } from "react";

import type { IconName } from "@/features/action-bar/lib/menu";

const icons: Record<IconName, ComponentType<LucideProps>> = {
  admin: ShieldCheck,
  film: Film,
  home: House,
  keyboard: Keyboard,
  refresh: RefreshCw,
  requests: Inbox,
  settings: Settings,
  tv: Tv,
};

/** The icon called `name`, at 14px. */
export function RowIcon({ name }: { name: IconName }) {
  const Icon = icons[name];
  return <Icon aria-hidden strokeWidth={1.5} className="size-3.5" />;
}
