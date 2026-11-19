import { Logo } from "@/shared/components/logo";

/** The Nama mark and name, over a signed-out page's card. */
export function Brand() {
  return (
    <div className="flex items-center justify-center gap-2.5">
      <Logo size={32} />
      <span className="text-lg/6 font-semibold tracking-tight">Nama</span>
    </div>
  );
}
