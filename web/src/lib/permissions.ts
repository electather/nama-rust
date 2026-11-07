import { Permission, PermissionSchema, type User } from "@/gen/nama/v1/sign_in_pb";

/** A Permission as the Users page offers it. */
export interface PermissionChoice {
  permission: Permission;
  label: string;
  /** What it lets a User do, beyond what its label says. */
  about?: string;
  /** The Permission that covers this one, which the Users page shows it under. */
  parent?: Permission;
}

/** Every Permission, in the order the Users page lists them. */
export const permissionChoices: PermissionChoice[] = [
  { permission: Permission.ADMIN, label: "Admin", about: "May do everything." },
  {
    permission: Permission.MANAGE_USERS,
    label: "Manage Users",
    about:
      "Create and delete Users, set their Permissions and passwords, and sign out their Devices.",
  },
  {
    permission: Permission.MANAGE_LIBRARIES,
    label: "Manage Libraries",
    about: "Add, change and scan Libraries, and fix their Matches.",
  },
  {
    permission: Permission.MANAGE_SETTINGS,
    label: "Manage settings",
    about: "Change the Server’s settings, such as its Plugins.",
  },
  {
    permission: Permission.MANAGE_REQUESTS,
    label: "Manage Requests",
    about: "Approve and decline Requests.",
  },
  { permission: Permission.REQUEST, label: "Request", about: "Request Movies and Shows." },
  { permission: Permission.REQUEST_MOVIE, label: "Request Movies", parent: Permission.REQUEST },
  { permission: Permission.REQUEST_TV, label: "Request Shows", parent: Permission.REQUEST },
  {
    permission: Permission.AUTO_APPROVE,
    label: "Auto-approve",
    about: "Their Requests are approved as they make them.",
  },
  {
    permission: Permission.AUTO_APPROVE_MOVIE,
    label: "Auto-approve Movies",
    parent: Permission.AUTO_APPROVE,
  },
  {
    permission: Permission.AUTO_APPROVE_TV,
    label: "Auto-approve Shows",
    parent: Permission.AUTO_APPROVE,
  },
  {
    permission: Permission.VIEW_REQUESTS,
    label: "View Requests",
    about: "See everyone’s Requests, not only their own.",
  },
];

/**
 * Whether another of `held` covers `permission`, as the Server checks it (ADR 0020): Admin covers
 * every other, and a parent its children.
 */
export function isCovered(held: Permission[], permission: Permission) {
  const { parent } = permissionChoices.find((choice) => choice.permission === permission) ?? {};
  return (
    permission !== Permission.ADMIN &&
    (held.includes(Permission.ADMIN) || (parent !== undefined && held.includes(parent)))
  );
}

/** Whether `user` may do what `permission` allows, by holding it or one that covers it. */
export function may(user: User | undefined, permission: Permission) {
  const held = user?.permissions ?? [];
  return held.includes(permission) || isCovered(held, permission);
}

/**
 * Whether `you` may change `user`'s Permissions, as the Server checks it: only an Admin changes their
 * own, or an Admin's.
 */
export function mayChangePermissions(you: User | undefined, user: User) {
  return may(you, Permission.ADMIN) || (you?.id !== user.id && !may(user, Permission.ADMIN));
}

/**
 * Whether `you` may change the default Permissions, as the Server checks it: only an Admin while
 * they hold Admin.
 */
export function mayChangeDefaultPermissions(you: User | undefined, defaults: Permission[]) {
  return may(you, Permission.ADMIN) || !defaults.includes(Permission.ADMIN);
}

/** The Permissions `you` may give: the Server refuses the Admin Permission from anyone without it. */
export function permissionsToOffer(you: User | undefined) {
  return may(you, Permission.ADMIN)
    ? permissionChoices
    : permissionChoices.filter((choice) => choice.permission !== Permission.ADMIN);
}

/** The label of the Permission a Refusal names by its proto name (`PERMISSION_MANAGE_USERS`). */
export function permissionLabel(name: string) {
  const value = PermissionSchema.values.find((each) => each.name === name)?.number;
  return permissionChoices.find((choice) => choice.permission === value)?.label;
}
