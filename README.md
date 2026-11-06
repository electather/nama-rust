# Nama

A self-hosted media server and request manager, with an optional central Hub for identity and
Server discovery. The vocabulary is in [`GLOSSARY.md`](GLOSSARY.md) and the decisions behind it
are in [`docs/adr/`](docs/adr/).

## Metadata from TMDB

A Server Matches the files in its Libraries to Movies and Shows using the built-in TMDB Plugin
(`plugins/tmdb`). It needs a TMDB API key, which you can get for free from
[themoviedb.org](https://www.themoviedb.org/settings/api). Put it in the Server's `nama.toml`:

```toml
[plugins.tmdb]
api_key = "your TMDB API key"
# language = "en-US"   # the language titles and overviews are fetched in
```

Without a key the Plugin isn't enabled and every file stays Unmatched.

## Attribution

This product uses the TMDB API but is not endorsed or certified by TMDB. Metadata and images
are provided by [The Movie Database (TMDB)](https://www.themoviedb.org).
