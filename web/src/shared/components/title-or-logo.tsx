import { useState } from "react";

import { useSrcSet } from "@/shared/hooks/use-src-set";

/**
 * A title as its Title Logo, named by the title, or as text without one or once it fails to load.
 * `logo` sizes the logo's box and `sizes` says how wide it shows, for its `srcset`.
 */
export function TitleOrLogo({
  title,
  logoUrl,
  logo,
}: {
  title: string;
  logoUrl: string | undefined;
  logo: { className: string; sizes: string };
}) {
  const srcSet = useSrcSet(logoUrl, "titleLogoWidths");
  // Kept by URL, so the next title's logo, shown in the same place, gets its own try.
  const [failedUrl, setFailedUrl] = useState<string>();
  if (logoUrl === undefined || logoUrl === failedUrl) {
    return title;
  }
  return (
    <img
      src={logoUrl}
      srcSet={srcSet}
      sizes={logo.sizes}
      alt={title}
      className={`w-auto object-contain object-left ${logo.className}`}
      onError={() => {
        setFailedUrl(logoUrl);
      }}
    />
  );
}
