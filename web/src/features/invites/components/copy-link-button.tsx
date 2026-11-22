import { Copy } from "lucide-react";
import { Button } from "nama-ui/components/button";
import { useState } from "react";

/** Copies `url`, or a command in its place, then says it did. `label` says what it copies. */
export function CopyLinkButton({ url, label = "Copy link" }: { url: string; label?: string }) {
  const [copied, setCopied] = useState(false);
  async function copy() {
    try {
      await navigator.clipboard.writeText(url);
      setCopied(true);
    } catch {
      // Unable to copy, the text can still be selected where it shows.
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
      {copied ? "Copied" : label}
    </Button>
  );
}
