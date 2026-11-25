import { configure } from "@testing-library/react";

// A file's first `findBy…` waits while the app's route chunks load for the first time, which can
// take over 5 s under a full parallel run (#2100), so it waits as long as `vite.config.ts`'s `expect.poll`.
configure({ asyncUtilTimeout: 10_000 });
