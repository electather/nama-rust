import { TitleLogo } from "@/features/title/components/title-logo";

/** The page's heading: the Title Logo, or else the title as text. */
export function TitleName({ title, logoUrl }: { title: string; logoUrl: string | undefined }) {
  return (
    <h1 className="text-display font-medium text-balance">
      {logoUrl === undefined ? title : <TitleLogo url={logoUrl} title={title} />}
    </h1>
  );
}
