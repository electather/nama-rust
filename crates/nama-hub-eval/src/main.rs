//! Scores the Hub's recommendations against MovieLens ratings, so changes to them can be compared
//! before Nama has Viewing Events of its own (ADR 0095).

mod movielens;
mod ranking;

use std::{
    collections::{HashMap, HashSet},
    env,
};

use anyhow::{Context as _, ensure};
use futures_util::{StreamExt as _, TryStreamExt as _, stream};
use nama_hub::{
    db,
    titles::{self, ExternalId, SimilarTitle, TitleKind},
};
use sqlx::PgPool;

use crate::ranking::{K, User};

/// The Similar answers asked at once: the pool's default size.
const CONCURRENT_QUERIES: usize = 10;

/// Trains the title vectors on the throwaway database `NAMA_HUB_DATABASE_URL` names, which
/// Wikidata's dump was imported into, then scores Similar titles against MovieLens's held-out
/// ratings. MovieLens is never written to the database.
#[tokio::main]
#[expect(clippy::print_stdout, reason = "the report is what the harness is for")]
async fn main() -> anyhow::Result<()> {
    let url = env::var("NAMA_HUB_DATABASE_URL")
        .context("NAMA_HUB_DATABASE_URL names no database Wikidata's dump was imported into")?;
    let db = db::connect(&url).await?;
    db::migrate(&db).await?;
    let vectors = titles::train(&db).await?;
    println!("trained {vectors} title vectors");

    let users = ranking::split(movielens::download()?);
    ensure!(
        !users.is_empty(),
        "no MovieLens user liked a film on both sides of their split"
    );
    let similar = similar_to_liked(&db, &users).await?;
    let scores = ranking::score(&users, &similar);
    println!(
        "{} MovieLens users: Recall@{K} {:.4}, NDCG@{K} {:.4}",
        scores.users, scores.recall, scores.ndcg
    );
    Ok(())
}

/// The Hub's Similar answer for each title a user liked before their split, by its IMDb id.
async fn similar_to_liked(
    db: &PgPool,
    users: &[User],
) -> Result<HashMap<String, Vec<SimilarTitle>>, sqlx::Error> {
    let liked: HashSet<&String> = users.iter().flat_map(|user| &user.liked).collect();
    stream::iter(liked)
        .map(|title| async move {
            let id = ExternalId {
                namespace: "imdb".to_owned(),
                value: title.clone(),
            };
            let alike = titles::similar(db, TitleKind::Movie, &[id]).await?;
            Ok((title.clone(), alike))
        })
        .buffer_unordered(CONCURRENT_QUERIES)
        .try_collect()
        .await
}
