import { useSearch } from "@tanstack/react-router";

/**
 * Names the browser tab after the page that renders it, then the app (WCAG 2.2 SC 2.4.2), unless
 * Settings is open over the page and names it instead.
 */
export function PageTitle({ name }: { name: string }) {
  // React 19 shows whichever `<title>` mounted last, and a page can mount after Settings, such as
  // once its loader is slower than Settings' code, so the page steps aside rather than racing it.
  const settingsOpen = useSearch({
    strict: false,
    select: ({ settings }) => settings !== undefined,
  });
  return settingsOpen ? null : <title>{`${name} – Nama`}</title>;
}
