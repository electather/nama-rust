import { QRCodeSVG } from "qrcode.react";

/** `code` as a link to `/code` on this Server, for a phone to open by scanning. */
export function ScanToApprove({ code }: { code: string }) {
  return (
    // Dark modules on a light ground, since not every scanner reads them inverted. That holds only
    // while the app is dark only: a light theme makes `foreground` dark.
    <QRCodeSVG
      value={`${globalThis.location.origin}/code?${new URLSearchParams({ code })}`}
      title="Scan it with your phone to approve this Device there"
      size={144}
      marginSize={4}
      bgColor="transparent"
      fgColor="currentColor"
      className="rounded-lg bg-foreground text-background"
    />
  );
}
