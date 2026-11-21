use std::{fs, path::PathBuf};

use nama_plugin_api::contract::{Artwork, ArtworkKind};

use super::{Images, ShowDetails};

fn fixture(name: &str) -> Vec<u8> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures/api")
        .join(name);
    fs::read(&path).unwrap_or_else(|err| panic!("{}: {err}", path.display()))
}

fn artwork(name: &str) -> Vec<Artwork> {
    serde_json::from_slice::<Images>(&fixture(name))
        .expect("an images response")
        .into()
}

fn image(
    kind: ArtworkKind,
    path: &str,
    language: Option<&str>,
    (width, height): (u32, u32),
    (rating, votes): (f64, u32),
) -> Artwork {
    Artwork {
        kind: kind.into(),
        path: path.to_owned(),
        language: language.map(str::to_owned),
        width: Some(width),
        height: Some(height),
        rating: Some(rating),
        votes: Some(votes),
        ..Default::default()
    }
}

#[test]
fn an_images_response_becomes_artwork_of_each_kind() {
    use ArtworkKind::{ARTWORK_KIND_BACKDROP, ARTWORK_KIND_LOGO, ARTWORK_KIND_POSTER};

    let expected = [
        image(
            ARTWORK_KIND_POSTER,
            "/apbrbWs8M9lyOpJYU5WXrpFbk1Z.jpg",
            Some("en"),
            (1000, 1500),
            (5.8, 16),
        ),
        image(
            ARTWORK_KIND_POSTER,
            "/w2bDDkGz0YtBJm1jHjXtPhT3zsn.jpg",
            Some("de"),
            (1000, 1500),
            (6.0, 6),
        ),
        image(
            ARTWORK_KIND_BACKDROP,
            "/3lBDg3i6nn5R2NKFCJ6oKyUo2j5.jpg",
            None,
            (1920, 1080),
            (5.4, 9),
        ),
        image(
            ARTWORK_KIND_BACKDROP,
            "/rPf5wX0tYTVuuJ8XFlXhcjnDTZy.jpg",
            Some("de"),
            (1920, 1080),
            (5.7, 4),
        ),
        image(
            ARTWORK_KIND_LOGO,
            "/b3eMvXwALgdBkBd2zZpDmpDWBbX.png",
            Some("en"),
            (600, 200),
            (5.3, 2),
        ),
        image(
            ARTWORK_KIND_LOGO,
            "/xjK8mXrpHL6wGZnn3SCRmKt0dT1.png",
            Some("de"),
            (600, 200),
            (5.6, 5),
        ),
    ];

    assert_eq!(artwork("tv-70523-images.json"), expected);
}

/// The artwork the pick rule takes of `kind`: English first, else textless, except a backdrop,
/// textless first, else English; by rating, then votes, then list order.
fn picked(artwork: &[Artwork], kind: ArtworkKind) -> Option<String> {
    let of_kind = || artwork.iter().filter(move |image| image.kind == kind);
    let english: Vec<_> = of_kind()
        .filter(|image| image.language.as_deref() == Some("en"))
        .collect();
    let textless: Vec<_> = of_kind().filter(|image| image.language.is_none()).collect();
    let (first, then) = if kind == ArtworkKind::ARTWORK_KIND_BACKDROP {
        (textless, english)
    } else {
        (english, textless)
    };
    let candidates = if first.is_empty() { then } else { first };
    candidates
        .into_iter()
        .min_by(|a, b| {
            b.rating
                .partial_cmp(&a.rating)
                .unwrap()
                .then(b.votes.cmp(&a.votes))
        })
        .map(|image| image.path.clone())
}

/// Matching keeps the picked poster and backdrop in place of a title's own, and the Server's
/// tests assert the own paths, whose bytes are the only ones in `fixtures/images/`.
#[test]
fn every_images_fixture_picks_its_titles_own_poster_and_backdrop() {
    for (images, details) in [
        ("movie-348-images.json", "movie-348.json"),
        ("movie-949-images.json", "movie-949.json"),
        ("tv-70523-images.json", "tv-70523.json"),
        ("tv-70523-season-1-images.json", "tv-70523-season-1.json"),
    ] {
        let artwork = artwork(images);
        let details: serde_json::Value = serde_json::from_slice(&fixture(details)).unwrap();
        for (kind, field) in [
            (ArtworkKind::ARTWORK_KIND_POSTER, "poster_path"),
            (ArtworkKind::ARTWORK_KIND_BACKDROP, "backdrop_path"),
        ] {
            assert_eq!(
                picked(&artwork, kind).as_deref(),
                details[field].as_str(),
                "{images}: {field}"
            );
        }
    }
}

#[test]
fn every_picked_logo_has_bytes_to_download() {
    for images in [
        "movie-348-images.json",
        "movie-949-images.json",
        "tv-70523-images.json",
    ] {
        if let Some(logo) = picked(&artwork(images), ArtworkKind::ARTWORK_KIND_LOGO) {
            let bytes = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("fixtures/images")
                .join(logo.trim_start_matches('/'));
            assert!(bytes.is_file(), "{images}: {}", bytes.display());
        }
    }
}

#[test]
fn a_title_tmdb_gives_no_language_has_no_original_language() {
    let silent = r#"{"id": 1, "name": "Silent", "original_language": "xx"}"#;

    let show = serde_json::from_str::<ShowDetails>(silent)
        .unwrap()
        .into_show(None);

    assert_eq!(show.original_language, None);
}
