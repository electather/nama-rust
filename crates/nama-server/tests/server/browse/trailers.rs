//! A Movie's and a Show's Trailers, and the thumbnails the Server serves for them.

use nama_proto::nama::v1::Trailer;

use super::*;

#[tokio::test]
async fn a_movie_and_a_show_have_only_their_trailers_as_links_out() {
    let movies = Fixture::alien().await;
    let shows = Fixture::shows().await;
    let urls = |trailers: &[Trailer]| -> Vec<String> {
        trailers.iter().map(|trailer| trailer.url.clone()).collect()
    };

    let alien = movies.get_movie(movies.library, ALIEN).await.unwrap();
    let dark = shows.get_show(shows.library, DARK).await.unwrap();
    let dark = dark.show.into_option().unwrap();

    assert_eq!(
        urls(&alien.trailers),
        [
            "https://www.youtube.com/watch?v=LjLamj-b0I8",
            "https://www.youtube.com/watch?v=fanTrail01"
        ]
    );
    assert_eq!(alien.trailers[0].name, "Official Trailer");
    assert_eq!(
        urls(&dark.trailers),
        [
            "https://www.youtube.com/watch?v=darkS1trail",
            "https://www.youtube.com/watch?v=darkS2trail",
            "https://www.youtube.com/watch?v=darkS3trail",
            "https://www.youtube.com/watch?v=darkFinal01",
            "https://vimeo.com/123456789"
        ]
    );
}

#[tokio::test]
async fn a_trailers_thumbnail_is_served_by_the_server_and_falls_back_to_hqdefault() {
    let fx = Fixture::alien().await;

    let alien = fx.get_movie(fx.library, ALIEN).await.unwrap();
    let thumbnail = alien.trailers[0].thumbnail_url.clone().unwrap();
    let reply = fetch(&fx.server, &thumbnail, Some(&fx.server.admin_token)).await;

    assert_eq!(
        thumbnail,
        "/media/artwork/tmdb/https%3A%2F%2Fi.ytimg.com%2Fvi%2FLjLamj-b0I8%2Fmaxresdefault.jpg"
    );
    // The fake YouTube has no `maxresdefault.jpg`, as for a video uploaded below 720p.
    assert_eq!(reply.status, 200);
    assert!(reply.body.ends_with(b"hqdefault thumbnail"));
}
