import { ChevronDown } from "lucide-react";
import { Button } from "nama-ui/components/button";
import { useId, useState } from "react";

/**
 * A "File details" button that reveals where the file is, `path`, under the buttons beside it. A
 * User who manages Libraries may see it (ADR 0069).
 */
export function FileDetails({ path }: { path: string }) {
  const panel = useId();
  const [open, setOpen] = useState(false);
  return (
    <>
      <Button
        type="button"
        variant="ghost"
        size="touch"
        aria-expanded={open}
        aria-controls={panel}
        onClick={() => {
          setOpen(!open);
        }}
      >
        File details
        <ChevronDown aria-hidden strokeWidth={1.5} className={open ? "rotate-180" : ""} />
      </Button>
      <p
        id={panel}
        hidden={!open}
        className="basis-full text-xs wrap-anywhere text-muted-foreground"
      >
        {path}
      </p>
    </>
  );
}
