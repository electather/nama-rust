import { useSearch } from "@tanstack/react-router";
import { Suspense, lazy } from "react";

import type { User } from "@/gen/nama/v1/sign_in_pb";

// Not on screen until it's asked for, so it stays out of the first page's code.
const SettingsModal = lazy(async () => {
  const module = await import("@/features/settings/components/settings-modal");
  return { default: module.SettingsModal };
});

/**
 * Settings, over whichever page `?settings=<section>` is on. Opening it is a history entry, so Back
 * closes it, and a link to that address opens it on its section.
 */
export function SettingsLoader({ user }: { user: User | undefined }) {
  const section = useSearch({ strict: false, select: ({ settings }) => settings });
  if (section === undefined) {
    return null;
  }
  return (
    <Suspense fallback={null}>
      <SettingsModal section={section} user={user} />
    </Suspense>
  );
}
