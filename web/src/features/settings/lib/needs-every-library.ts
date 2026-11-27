import { Permission } from "@/gen/nama/v1/sign_in_pb";
import { holds } from "@/shared/lib/permissions";

/**
 * Whether a User holding `held` must see every Library, as the Server checks it
 * (`crates/nama-server/src/users/AGENTS.md`, "Managing Users needs every Library").
 */
export function needsEveryLibrary(held: Permission[]) {
  return holds(held, Permission.MANAGE_USERS) && !holds(held, Permission.MANAGE_LIBRARIES);
}
