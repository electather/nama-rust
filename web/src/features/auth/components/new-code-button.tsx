import { RefreshCw } from "lucide-react";
import { Button } from "nama-ui/components/button";

/** Gets a new Sign-in Code once the last one expired. */
export function NewCodeButton({ pending, onClick }: { pending: boolean; onClick: () => void }) {
  return (
    <Button size="lg" disabled={pending} onClick={onClick}>
      <RefreshCw aria-hidden />
      {pending ? "Getting a code…" : "Get a new code"}
    </Button>
  );
}
