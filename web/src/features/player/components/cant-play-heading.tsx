/** The title, that its Version can't play here, and `line` under it, such as a general why. */
export function CantPlayHeading({ title, line }: { title: string; line: string | undefined }) {
  return (
    <div className="flex flex-col gap-2">
      <p className="text-sm">{title}</p>
      <h1 className="text-3xl font-medium tracking-tight text-balance">
        This Version can’t play here
      </h1>
      {line !== undefined && <p className="leading-7 text-pretty">{line}</p>}
    </div>
  );
}
