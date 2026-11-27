import type { LucideIcon } from "lucide-react";

/** A section's `icon`, in the ink of the open one when it's `marked`. */
export function SectionIcon({ icon: Icon, marked }: { icon: LucideIcon; marked: boolean }) {
  return (
    <Icon aria-hidden strokeWidth={1.5} className={marked ? "size-4 text-primary" : "size-4"} />
  );
}
