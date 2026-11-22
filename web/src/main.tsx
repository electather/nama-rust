import { RouterProvider } from "@tanstack/react-router";
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

// oxlint-disable-next-line import/no-unassigned-import -- Vite links the app's stylesheet from `index.html` for this import
import "./styles.css";
// oxlint-disable-next-line import/no-unassigned-import -- Vite links the text subtitle cues' stylesheet from `index.html` for this import
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
