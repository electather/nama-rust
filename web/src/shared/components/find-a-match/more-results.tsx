import { ChevronRight } from "lucide-react";
import { Button, ghostHang } from "nama-ui/components/button";

/** The "N more results" button that `onShow` the titles past the best fits. */
export function MoreResults({ more, onShow }: { more: number; onShow: () => void }) {
  return (
    <div className={ghostHang}>
      <Button type="button" variant="ghost" size="touch" aria-expanded={false} onClick={onShow}>
        <ChevronRight aria-hidden strokeWidth={1.5} className="rtl:rotate-180" />
        {more} more {more === 1 ? "result" : "results"}
      </Button>
    </div>
  );
}
