import { ExternalTextLink } from "@/shared/components/external-text-link";

/** The link to finish connecting in the other app, in a new tab. */
export function OpenLink({ href, name }: { href: string; name: string }) {
  return (
    <ExternalTextLink href={href} className="rounded-lg px-4 py-2 text-sm font-medium">
      Open {name}
    </ExternalTextLink>
  );
}
