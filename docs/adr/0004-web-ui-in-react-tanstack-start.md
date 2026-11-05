# Web UI in React + TanStack Start, not Rust

The project is otherwise all Rust, but the web UI is TypeScript, React and TanStack Start. Most of the browser video tooling (hls.js, Media Source Extensions helpers, ASS subtitle renderers) is JavaScript, and a Rust/WASM UI framework would mean wrapping all of it by hand. TanStack Start runs in SPA mode: the build output (one prerendered `_shell.html` plus static assets) is embedded in the Server binary, so there's no Node process at runtime and no server-side rendering.
