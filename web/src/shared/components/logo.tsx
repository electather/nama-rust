import logo from "@/shared/components/logo.svg";

/**
 * The Nama mark, `size` px square. An image, not inline SVG, so its gradient ids can't clash with
 * the page's. Give it a `label` only where no written name stands beside it.
 */
export function Logo({ size, label = "" }: { size: number; label?: string }) {
  return (
    <img src={logo} width={size} height={size} alt={label} draggable={false} className="shrink-0" />
  );
}
