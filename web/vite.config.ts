import tailwindcss from "@tailwindcss/vite";
import { tanstackRouter } from "@tanstack/router-plugin/vite";
import viteReact from "@vitejs/plugin-react";
import { defineConfig } from "vite-plus";

// The Server `vp dev` forwards RPCs and media to, so the browser sees one origin (ADR 0015).
const serverUrl = process.env["NAMA_SERVER_URL"] ?? "http://localhost:7980";

// CI sets `CI`. There a failed test is retried twice and the results go to a report that
// `nama-test-report` reads (ADR 0022); locally a flaky test fails, so you notice it.
const ci = Boolean(process.env["CI"]);

// Written by `buf generate` and the router plugin, never by hand.
const generated = ["src/gen/**", "src/routeTree.gen.ts"];

// The feature layout's folders (web/src/AGENTS.md) and nama-ui, where the strictest rules apply.
const newFolders = ["src/app/**", "src/features/**", "src/shared/**", "ui/src/**"];

export default defineConfig({
  resolve: { tsconfigPaths: true },
  // The build's `index.html` is what the Server embeds and serves for every page (ADR 0004). The
  // router plugin goes before React's, which would otherwise transform the routes first.
  plugins: [
    tailwindcss(),
    tanstackRouter({ target: "react", autoCodeSplitting: true }),
    viteReact(),
  ],
  server: {
    proxy: {
      // Connect paths (`/nama.v1.ServerService/GetServerInfo` and the like) and media: the same
      // paths the Server keeps out of the web app (`crates/nama-server/src/web.rs`). Custom Images
      // are uploaded with `PUT` and removed with `DELETE`, which the web app never answers, and
      // the web app's Diagnostics are posted.
      "^/nama\\.": serverUrl,
      "/media/": serverUrl,
      "/custom-images/": serverUrl,
      "/diagnostics/": serverUrl,
    },
  },
  test: {
    // Tests live in `__tests__` folders.
    include: ["src/**/__tests__/**/*.test.{ts,tsx}"],
    environment: "happy-dom",
    retry: ci ? 2 : 0,
    // Under a full run's load the app can take over the default 1 s to send its first RPC (`src/test-app.tsx` raises Testing Library's wait to match).
    expect: { poll: { timeout: 5000 } },
    // Over the 5 s waits, so one that runs out names what it missed; a cold run's first imports take seconds.
    testTimeout: 15_000,
    // JSON, not JUnit: vitest's JUnit report doesn't show which tests passed only on a retry.
    reporters: ci ? ["default", "json"] : ["default"],
    outputFile: { json: "test-results/vitest.json" },
  },
  // The formatter owns import and class order, so no lint rule competes with it.
  fmt: {
    ignorePatterns: generated,
    sortImports: true,
    sortTailwindcss: { stylesheet: "src/styles.css", functions: ["cn", "cva"] },
  },
  lint: {
    ignorePatterns: generated,
    plugins: ["typescript", "react", "jsx-a11y", "import", "unicorn", "promise", "oxc", "vitest"],
    jsPlugins: [
      // Runs inside oxlint, so it doesn't break "Only `vp`" (web/AGENTS.md).
      "eslint-plugin-better-tailwindcss",
      { name: "vite-plus", specifier: "vite-plus/oxlint-plugin" },
      // Design-system rules, read from the nearest components.json and the stylesheet it names.
      { name: "shadcn", specifier: "@shadcn/lint" },
    ],
    // Nursery rules change between oxlint versions. vite-plus is pinned in the workspace catalog,
    // so an upgrade is a deliberate PR that fixes whatever new nursery hits appear.
    categories: {
      correctness: "error",
      suspicious: "error",
      pedantic: "error",
      perf: "error",
      style: "error",
      restriction: "error",
      nursery: "error",
    },
    env: { browser: true, es2024: true },
    settings: {
      "better-tailwindcss": { entryPoint: "src/styles.css" },
    },
    rules: {
      "vite-plus/prefer-vite-plus-imports": "error",
      "shadcn/no-restyle": "error",
      "shadcn/no-raw-colors": "error",
      "shadcn/no-arbitrary-values": "error",
      "shadcn/no-inline-styles": "error",
      "shadcn/no-unknown-classes": "error",
      "shadcn/require-static-classes": "error",
      "better-tailwindcss/no-conflicting-classes": "error",
      "better-tailwindcss/no-duplicate-classes": "error",
      "better-tailwindcss/no-deprecated-classes": "error",
      "better-tailwindcss/no-unnecessary-whitespace": "error",
      "better-tailwindcss/no-concatenated-classes": "error",
      "better-tailwindcss/enforce-canonical-classes": "error",
      "better-tailwindcss/enforce-shorthand-classes": "error",
      // web/AGENTS.md, "Logical properties": only left and right. Block-axis and sizing classes
      // (`mt-*`, `top-*`, `border-b`, `w-full`, `size-4`) stay, since `mbs-*` and `inline-full`
      // would change nothing for right-to-left. A pattern sees the class with its variants.
      "better-tailwindcss/enforce-logical-properties": [
        "error",
        {
          ignore: [
            "(^|:)-?(scroll-)?[mp][tb]-",
            "(^|:)-?(top|bottom)-",
            "(^|:)border-[tb](-|$)",
            "(^|:)(min-|max-)?[hw]-",
            "(^|:)size-",
          ],
        },
      ],
      // `enforce-consistent-class-order` is left off: the formatter sorts classes.
      // `enforce-consistent-line-wrapping` is left off: it fights the formatter.
      "func-style": ["error", "declaration", { allowArrowFunctions: true }],
      curly: ["error", "all"],
      // `void promise` marks a promise deliberately not awaited; nothing else uses `void`.
      "no-void": ["error", { allowAsStatement: true }],
      "oxc/no-barrel-file": ["error", { threshold: 0 }],
      // A mixed import marks its types inline. An all-type import stays `import type`, since
      // `import { type A }` leaves a side-effect import behind under `verbatimModuleSyntax`
      // (`no-import-type-side-effects`). So `consistent-type-specifier-style` stays off: its
      // `prefer-inline` rewrites every `import type` into exactly that.
      "typescript/consistent-type-imports": ["error", { fixStyle: "inline-type-imports" }],
      "import/consistent-type-specifier-style": "off",
      "typescript/consistent-type-definitions": ["error", "interface"],
      // `$param` route files are named by TanStack Router.
      "unicorn/filename-case": ["error", { case: "kebabCase", ignore: [String.raw`^\$`] }],
      "react/jsx-filename-extension": ["error", { extensions: [".tsx"] }],
      // Tests are flat `test`s, inside a `describe` too.
      "vitest/consistent-test-it": ["error", { fn: "test", withinDescribe: "test" }],
      "typescript/no-explicit-any": "error",
      "typescript/no-non-null-assertion": "error",
      // The router's `redirect()` and `notFound()` are thrown, and aren't `Error`s.
      "typescript/only-throw-error": [
        "error",
        {
          allow: [
            {
              from: "package",
              package: "@tanstack/router-core",
              name: ["Redirect", "NotFoundError"],
            },
          ],
        },
      ],
      // `expectUnderField` (src/test-refusal.ts) asserts for the tests that call it.
      "vitest/expect-expect": ["error", { assertFunctionNames: ["expect", "expectUnderField"] }],
      // The automatic JSX runtime needs no `React` in scope.
      "react/react-in-jsx-scope": "off",
      // React props (`children: ReactNode`) and library callback arguments can't be deeply
      // readonly, so this flags every component and callback.
      "typescript/prefer-readonly-parameter-types": "off",
      // The formatter sorts imports.
      "sort-imports": "off",
      // Taste only: strictness should buy bug-catching, not churn.
      "one-var": "off",
      "sort-keys": "off",
      "no-ternary": "off",
      "init-declarations": "off",
      "id-length": "off",
      // Mostly test data.
      "no-magic-numbers": "off",
      // Its fix breaks `ponytail:` markers and comments that start with an identifier.
      "capitalized-comments": "off",
      // They contradict named exports.
      "import/no-named-export": "off",
      "import/prefer-default-export": "off",
      "import/group-exports": "off",
      "import/exports-last": "off",
      // React, the DOM and protobuf all use `null`.
      "unicorn/no-null": "off",
      // `exactOptionalPropertyTypes` needs `undefined`.
      "no-undefined": "off",
      // Files read top-down: a component first, the helpers it uses after.
      "no-use-before-define": "off",
      // shadcn's components pass `{...props}` on.
      "react/jsx-props-no-spreading": "off",
      // shadcn's `no-restyle` covers it.
      "react/forbid-component-props": "off",
      // Fallow's complexity and file-size limits cover them.
      "max-statements": "off",
      "import/max-dependencies": "off",
      "max-lines": "off",
      "max-lines-per-function": "off",
      "vitest/max-expects": "off",
      // Callback APIs (timers, events, `<img>` loading) have to be wrapped in a promise.
      "promise/avoid-new": "off",
      // Rules for old ES targets.
      "oxc/no-async-await": "off",
      "oxc/no-optional-chaining": "off",
      "oxc/no-rest-spread-properties": "off",
      // They contradict how the tests are written: flat `test`s, `beforeEach` setup, imported
      // globals, and Vitest's default timeout.
      "vitest/require-top-level-describe": "off",
      "vitest/prefer-expect-assertions": "off",
      "vitest/require-test-timeout": "off",
      "vitest/no-hooks": "off",
      "vitest/no-importing-vitest-globals": "off",
      // They conflict with `prefer-strict-boolean-matchers`.
      "vitest/prefer-to-be-truthy": "off",
      "vitest/prefer-to-be-falsy": "off",
      // React Compiler port messages, not findings. (`react_perf` isn't loaded: inline handlers
      // are fine in React 19.)
      "react/todo": "off",
      // Components return JSX, and the router's types can't be written out by hand.
      "typescript/explicit-function-return-type": "off",
      "typescript/explicit-module-boundary-types": "off",
      // Comes with the PR that adds translations.
      "react/jsx-no-literals": "off",
      // web/src/AGENTS.md, "One component per file".
      "react/no-multi-comp": ["error", { ignoreStateless: false }],
      // Need a person to fix every hit in the route files and test helpers, so they're on only in
      // the feature layout's folders (the overrides below).
      "react/only-export-components": "off",
      "react/jsx-handler-names": "off",
      "react/jsx-max-depth": "off",
      "typescript/promise-function-async": "off",
      "typescript/explicit-member-accessibility": "off",
      "max-params": "off",
      "prefer-named-capture-group": "off",
      "class-methods-use-this": "off",
      "no-empty-function": "off",
      "unicorn/prefer-global-this": "off",
      "unicorn/no-await-expression-member": "off",
      "unicorn/no-array-reduce": "off",
      "promise/prefer-await-to-then": "off",
      "vitest/prefer-strict-equal": "off",
      "vitest/require-hook": "off",
    },
    overrides: [
      // web/src/AGENTS.md, "Strictest lint": the strictest bar the tools offer.
      {
        files: newFolders,
        rules: {
          // Fast Refresh keeps a file's state only when it exports nothing but components. Route
          // files, which export `Route`, stay in src/routes/.
          "react/only-export-components": "error",
          "react/jsx-handler-names": "error",
          "react/jsx-max-depth": "error",
          // `satisfies` or a type guard instead.
          "typescript/consistent-type-assertions": ["error", { assertionStyle: "never" }],
          "typescript/promise-function-async": "error",
          "typescript/explicit-member-accessibility": "error",
          // `""` and `0` aren't conditions. Checking whether an object is there still is.
          "typescript/strict-boolean-expressions": [
            "error",
            { allowString: false, allowNumber: false, allowNullableObject: true },
          ],
          "max-params": "error",
          "prefer-named-capture-group": "error",
          "class-methods-use-this": "error",
          "no-empty-function": "error",
          "unicorn/prefer-global-this": "error",
          "unicorn/no-await-expression-member": "error",
          "unicorn/no-array-reduce": "error",
          "promise/prefer-await-to-then": "error",
          "vitest/prefer-strict-equal": "error",
          "vitest/require-hook": "error",
        },
      },
      // shadcn generates a component with its sub-parts and variants in one file, and a re-run of
      // `shadcn add` is diffed against ours (ui/src/components/AGENTS.md).
      {
        files: ["ui/src/components/**"],
        rules: {
          "react/no-multi-comp": "off",
          "react/only-export-components": "off",
          "react/jsx-max-depth": "off",
        },
      },
      // Wrapper and fixture components stay inline in a test, and a fake may do nothing.
      {
        files: ["src/**/__tests__/**"],
        rules: { "react/no-multi-comp": "off", "no-empty-function": "off" },
      },
      // An ambient module declaration (`declare module "…"`) has to be a script.
      { files: ["src/*.d.ts"], rules: { "import/unambiguous": "off" } },
      // Vite reads its config's default export, in Node.
      {
        files: ["vite.config.ts"],
        env: { node: true },
        rules: { "import/no-default-export": "off", "import/no-nodejs-modules": "off" },
      },
    ],
    options: { typeAware: true, typeCheck: true },
  },
});
