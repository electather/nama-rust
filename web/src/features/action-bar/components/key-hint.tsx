import type { ReactNode } from "react";

import { Kbd } from "@/features/action-bar/components/kbd";

/** Keys and what they do, for the action bar's footer. */
export function KeyHint({
  keys,
  label,
}: {
  keys: { name: string; key: ReactNode }[];
  label: string;
}) {
  return (
    <span className="inline-flex items-center gap-1.5">
      {keys.map(({ name, key }) => (
        <Kbd key={name}>{key}</Kbd>
      ))}
      {label}
    </span>
  );
}
