# nama-web

The web app the Server and the Hub serve: `build.rs` embeds `web/dist`, and `serve` answers with it. A new file type the app ships gets its arm in `content_type` in `src/lib.rs`, once for both.

Everything sits behind the `embed` feature, which needs the built app:

```sh
vp -C web build
cargo nextest run -p nama-web --features embed
```
