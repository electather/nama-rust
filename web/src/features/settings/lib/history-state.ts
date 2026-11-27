import { allSections } from "@/features/settings/lib/sections";

// What a phone's list of sections hands the section it opens, kept per history entry.
declare module "@tanstack/history" {
  interface HistoryState {
    /** The section was opened from the list of sections, so going back to it is Back. */
    fromSettingsList?: boolean;
  }
}

/**
 * How a section opens from the place `settings` names, `phone` or not. Wider, the sections share
 * one history entry, so Back closes Settings; on a phone one opened from the list of them gets an
 * entry of its own, so Back returns to the list.
 */
export function openingSection(settings: unknown, phone: boolean) {
  return phone && settings === allSections
    ? { replace: false, state: { fromSettingsList: true } }
    : { replace: true };
}
