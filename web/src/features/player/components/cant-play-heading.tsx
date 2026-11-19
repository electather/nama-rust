/** The title, that its Version can't play here, and when no reason says why, a general why. */
export function CantPlayHeading({ title, general }: { title: string; general: boolean }) {
  return (
    <div className="flex flex-col gap-2">
      <p className="text-sm">{title}</p>
      <h1 className="text-3xl font-medium tracking-tight text-balance">
        This Version can’t play here
      </h1>
      {general && (
        <p className="leading-7 text-pretty">
          This browser can’t play it. Another browser, or another Version, may play.
        </p>
      )}
    </div>
  );
}
