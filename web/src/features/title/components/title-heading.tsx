import { TitleName } from "@/features/title/components/title-name";

/** The title's heading, on a gradient that fades the backdrop into the page. */
export function TitleHeading({ title, logoUrl }: { title: string; logoUrl: string | undefined }) {
  return (
    <div className="group-data-art:bg-linear-to-t group-data-art:from-background group-data-art:to-scrim">
      <div className="mx-auto max-w-5xl px-4 pb-6 sm:px-6">
        <TitleName title={title} logoUrl={logoUrl} />
      </div>
    </div>
  );
}
