/** A Hub page's heading, which also names the browser tab (WCAG 2.2 SC 2.4.2). */
export function PageHeading({ children }: { children: string }) {
  return (
    <>
      {/* React 19 hoists it into `<head>`, ahead of the layout's own. */}
      <title>{`${children} – Nama Hub`}</title>
      <h1 className="text-center text-3xl font-medium tracking-tight text-balance">{children}</h1>
    </>
  );
}
