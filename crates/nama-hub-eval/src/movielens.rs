//! MovieLens's ratings, downloaded at run time.

use std::{collections::HashMap, path::Path, process::Command};

use anyhow::{Context as _, anyhow, ensure};

/// The small MovieLens dataset: 100,000 ratings of 9,000 films by 600 users. Downloaded, never
/// committed or built into the Hub, since its licence is for research only (ADR 0095).
const URL: &str = "https://files.grouplens.org/datasets/movielens/ml-latest-small.zip";

/// One MovieLens user's rating of a film.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Rating {
    /// The MovieLens user who rated it.
    pub(crate) user: u32,
    /// The film's IMDb id, such as `tt0114709`.
    pub(crate) title: String,
    /// From 0.5 to 5 stars.
    pub(crate) stars: f32,
    /// When, in seconds since the Unix epoch.
    pub(crate) time: i64,
}

/// Downloads MovieLens's ratings, with `curl` and `unzip`, into a temporary folder it then
/// deletes.
///
/// # Errors
///
/// If either command fails or its files don't parse.
pub(crate) fn download() -> anyhow::Result<Vec<Rating>> {
    let folder = tempfile::tempdir()?;
    let zip = folder.path().join("movielens.zip");
    run(Command::new("curl")
        .args([
            "--fail",
            "--silent",
            "--show-error",
            "--location",
            "--output",
        ])
        .arg(&zip)
        .arg(URL))?;
    parse(&unzip(&zip, "links.csv")?, &unzip(&zip, "ratings.csv")?)
}

fn unzip(zip: &Path, file: &str) -> anyhow::Result<String> {
    let contents = run(Command::new("unzip")
        .arg("-p")
        .arg(zip)
        .arg(format!("ml-latest-small/{file}")))?;
    Ok(String::from_utf8(contents)?)
}

/// Runs `command` and returns what it wrote to stdout.
fn run(command: &mut Command) -> anyhow::Result<Vec<u8>> {
    let output = command
        .output()
        .with_context(|| format!("could not run {command:?}"))?;
    ensure!(
        output.status.success(),
        "{command:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(output.stdout)
}

/// The ratings in `ratings.csv`, each naming its film by the IMDb id `links.csv` gives it.
///
/// MovieLens quotes no field in either file, so a comma always separates two.
fn parse(links: &str, ratings: &str) -> anyhow::Result<Vec<Rating>> {
    let mut imdb_ids = HashMap::new();
    for line in links.lines().skip(1) {
        let [movie, imdb, _tmdb] = fields(line)?;
        imdb_ids.insert(movie, format!("tt{imdb}"));
    }
    ratings
        .lines()
        .skip(1)
        .map(|line| {
            let [user, movie, stars, time] = fields(line)?;
            Ok(Rating {
                user: user.parse()?,
                title: imdb_ids
                    .get(movie)
                    .with_context(|| format!("no link for movie {movie}"))?
                    .clone(),
                stars: stars.parse()?,
                time: time.parse()?,
            })
        })
        .collect()
}

fn fields<const N: usize>(line: &str) -> anyhow::Result<[&str; N]> {
    let fields: Vec<&str> = line.split(',').collect();
    fields
        .try_into()
        .map_err(|fields: Vec<&str>| anyhow!("{} fields, not {N}: {line:?}", fields.len()))
}

#[cfg(test)]
mod tests {
    use super::{Rating, parse};

    #[test]
    fn ratings_name_their_film_by_imdb_id() {
        let links = "movieId,imdbId,tmdbId\n1,0114709,862\n2,0113497,\n";
        let ratings = "userId,movieId,rating,timestamp\n1,2,3.5,964982703\n7,1,5.0,964981247\n";

        let parsed = parse(links, ratings).expect("parses");

        assert_eq!(
            parsed,
            [
                Rating {
                    user: 1,
                    title: "tt0113497".to_owned(),
                    stars: 3.5,
                    time: 964_982_703
                },
                Rating {
                    user: 7,
                    title: "tt0114709".to_owned(),
                    stars: 5.0,
                    time: 964_981_247
                },
            ]
        );
    }

    #[test]
    fn a_rating_of_a_film_without_a_link_is_an_error() {
        let links = "movieId,imdbId,tmdbId\n1,0114709,862\n";
        let ratings = "userId,movieId,rating,timestamp\n1,2,3.5,964982703\n";

        let parsed = parse(links, ratings);

        assert!(parsed.is_err(), "{parsed:?}");
    }
}
