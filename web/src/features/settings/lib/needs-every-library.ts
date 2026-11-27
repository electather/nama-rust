import { Permission } from "@/gen/nama/v1/sign_in_pb";
import { isCovered } from "@/shared/lib/permissions";

/**
 * Whether a User holding `held` must see every Library, as the Server checks it
 * (`crates/nama-server/src/users/AGENTS.md`, "Managing Users needs every Library").
 */
export function needsEveryLibrary(held: Permission[]) {
  const holds = (permission: Permission) =>
    held.includes(permission) || isCovered(held, permission);
  return holds(Permission.MANAGE_USERS) && !holds(Permission.MANAGE_LIBRARIES);
}
