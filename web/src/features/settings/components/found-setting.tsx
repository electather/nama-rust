import type { RefObject } from "react";

import { useFoundSetting } from "@/features/settings/hooks/use-found-setting";
import type { Section } from "@/features/settings/lib/sections";

/**
 * Lands on the setting the address names in the `pane` of `section`. It draws nothing: it sits
 * beside the pane's settings inside their Suspense boundary, so it mounts once they've loaded.
 */
export function FoundSetting({
  pane,
  section,
}: {
  pane: RefObject<HTMLElement | null>;
  section: Section;
}) {
  useFoundSetting(pane, section);
  return null;
}
