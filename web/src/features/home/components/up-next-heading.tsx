import { SectionHeading } from "@/features/home/components/section-heading";

/** Continue Watching's heading, over "Up Next" in faint type as wide as the column allows. */
export function UpNextHeading({ id }: { id: string }) {
  return (
    <div className="@container relative isolate">
      <span
        aria-hidden
        className="pointer-events-none absolute inset-x-0 top-0 -z-10 overflow-hidden text-center text-watermark font-extrabold whitespace-nowrap text-foreground/10 select-none"
      >
        Up Next
      </span>
      <div className="pt-watermark">
        <SectionHeading id={id}>Continue Watching</SectionHeading>
      </div>
    </div>
  );
}
