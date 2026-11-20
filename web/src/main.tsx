import { RouterProvider } from "@tanstack/react-router";
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

// oxlint-disable-next-line import/no-unassigned-import -- Vite links the app's stylesheet from `index.html` for this import
import "./styles.css";
// oxlint-disable-next-line import/no-unassigned-import -- text subtitle cues, a stylesheet of its own since styles.css is at its line limit
import "./subtitle-cues.css";
import { getRouter } from "./router";

const app = document.querySelector("#app");
if (app) {
  createRoot(app).render(
    <StrictMode>
      <RouterProvider router={getRouter()} />
    </StrictMode>,
  );
}
