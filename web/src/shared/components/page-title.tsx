/** Names the browser tab after the page that renders it, then the app (WCAG 2.2 SC 2.4.2). */
export function PageTitle({ name }: { name: string }) {
  // React 19 hoists it into `<head>`, newest first, so Settings, mounted over a page, names the tab.
  return <title>{`${name} – Nama`}</title>;
}
