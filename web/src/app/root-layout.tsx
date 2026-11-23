import { Outlet } from "@tanstack/react-router";

/** Every page, and the tab's name while none names it, such as while one loads. */
export function RootLayout() {
  return (
    <>
      <title>Nama</title>
      <Outlet />
    </>
  );
}
