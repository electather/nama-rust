import { FooterKey } from "@/features/action-bar/components/footer-key";
import { Logo } from "@/shared/components/logo";

/** The action bar's foot: the keys that work in it, where there's a keyboard, and the mark. */
export function ActionBarFooter() {
  return (
    <footer className="flex items-center justify-between gap-3 border-t bg-card/40 px-3 py-2 text-xs text-muted-foreground">
      <div className="hidden gap-3.5 pointer-fine:inline-flex">
        <FooterKey keys={["up", "down"]} label="navigate" />
        <FooterKey keys={["enter"]} label="select" />
        <FooterKey keys={["esc"]} label="close" />
      </div>
      <span className="ms-auto">
        <Logo size={24} label="Nama" />
      </span>
    </footer>
  );
}
