import { fireEvent, screen, waitFor } from "@testing-library/react";
import { expect } from "vite-plus/test";

/** Opens the menu of the "…" named `name`, once its code has loaded and it's enabled. */
export async function openMenu(name: string) {
  const more = await waitFor(() => {
    const button = screen.getByRole("button", { name });
    expect(button.hasAttribute("disabled")).toBe(false);
    return button;
  });
  fireEvent.click(more);
}

/** Picks the item named `item` from the menu of the "…" named `name`. */
export async function pickFromMenu(name: string, item: string | RegExp) {
  await openMenu(name);
  fireEvent.click(await screen.findByRole("menuitem", { name: item }));
}
