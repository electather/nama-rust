import { Copy } from "lucide-react";
import { Button } from "nama-ui/components/button";
import { useState } from "react";

/** Copies `url`, then says it did. */
export function CopyLinkButton({ url }: { url: string }) {
  const [copied, setCopied] = useState(false);
  async function copy() {
    try {
      await navigator.clipboard.writeText(url);
      setCopied(true);
    } catch {
      // Unable to copy, the link can still be selected in the field.
    }
  }
  return (
    <Button
      variant="ghost"
      onClick={() => {
        void copy();
      }}
    >
      <Copy aria-hidden className="size-4" />
      {copied ? "Copied" : "Copy link"}
    </Button>
  );
}
