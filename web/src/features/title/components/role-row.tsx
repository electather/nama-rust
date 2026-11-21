import { RoleCard } from "@/features/title/components/role-card";
import type { Role } from "@/gen/nama/v1/request_pb";

/** `roles` in one row that scrolls sideways, out to the screen's edges as the Episode row does. */
export function RoleRow({ roles }: { roles: Role[] }) {
  return (
    <div
      // oxlint-disable-next-line jsx-a11y/no-noninteractive-tabindex -- a row that scrolls must take focus for keyboard users to scroll it (WCAG 2.1.1)
      tabIndex={0}
      // A swipe at its end stays on the row, so it doesn't go back a page.
      className="mx-bleed snap-x snap-proximity scroll-px-shelf-edge scrollbar-none overflow-x-auto overscroll-x-contain pt-1 pb-2 outline-none focus-visible:ring-3 focus-visible:ring-ring/50"
    >
      <ul aria-label="Cast & Crew" className="flex w-max gap-5 px-shelf-edge">
        {roles.map((role) => (
          <RoleCard key={`${role.name} ${role.part}`} role={role} />
        ))}
      </ul>
    </div>
  );
}
