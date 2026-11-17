//! Paging a long client list by keyset (ADR 0055): one place to clamp a page's size and to make and
//! check the tokens that say where the next page starts.

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use nama_proto::nama::v1::{PageRequest, PageResponse};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use sha2::{Digest as _, Sha256};

/// The size of a page a client names none for.
const DEFAULT_SIZE: u32 = 50;
/// The largest page a client gets, whatever size it names.
const MAX_SIZE: u32 = 100;

/// The page of a list a client asked for, sorted by a key of type `K` and tie-broken by id.
#[derive(Debug, PartialEq)]
pub(crate) struct Page<K> {
    /// How many rows the page lists at most.
    pub(crate) size: u32,
    /// The sort key and id of the last row of the page before, which this page lists after, or
    /// `None` for the first page.
    pub(crate) after: Option<(K, i64)>,
    /// The hash of the list, filter and sort the page is read under.
    list_hash: Hash,
}

/// A hash of a list, its filter and its sort, which a page token must have been made under.
type Hash = [u8; 8];

/// What a page token holds, encoded so clients treat it as opaque and don't build their own.
#[derive(Serialize, Deserialize)]
struct Cursor<K> {
    key: K,
    id: i64,
    list_hash: Hash,
}

/// A page token that isn't one the Server made for the same list, filter and sort.
#[derive(Debug, thiserror::Error)]
#[error("the page token isn't one this Server gave for this list, filter and sort")]
pub(crate) struct TokenInvalid;

impl<K: Serialize + DeserializeOwned> Page<K> {
    /// Reads `request` for the list `list` names with its filter and sort: two lists, filters or
    /// sorts must give different bytes, so a token made under one is refused under another.
    pub(crate) fn read(request: &PageRequest, list: &[u8]) -> Result<Self, TokenInvalid> {
        let list_hash = hash(list);
        let size = u32::try_from(request.size)
            .ok()
            .filter(|&size| size > 0)
            .map_or(DEFAULT_SIZE, |size| size.min(MAX_SIZE));
        let after = if request.token.is_empty() {
            None
        } else {
            let cursor = URL_SAFE_NO_PAD
                .decode(&request.token)
                .ok()
                .and_then(|json| serde_json::from_slice::<Cursor<K>>(&json).ok())
                .filter(|cursor| cursor.list_hash == list_hash)
                .ok_or(TokenInvalid)?;
            Some((cursor.key, cursor.id))
        };
        Ok(Self {
            size,
            after,
            list_hash,
        })
    }

    /// Whether this is the first page, the one that carries the list's `total_size`.
    pub(crate) fn is_first(&self) -> bool {
        self.after.is_none()
    }

    /// The SQL `LIMIT` to read this page with: one row more than it lists, which tells
    /// [`Self::finish`] whether a page follows.
    pub(crate) fn limit(&self) -> u32 {
        self.size + 1
    }

    /// Cuts `rows`, read in the list's order with [`Self::limit`], to this page, and gives the
    /// token for the next. `place` is a row's sort key and id.
    #[expect(
        clippy::expect_used,
        reason = "the sort keys used are plain values, which always serialize to JSON"
    )]
    pub(crate) fn finish<R>(
        &self,
        rows: &mut Vec<R>,
        place: impl FnOnce(&R) -> (K, i64),
    ) -> PageResponse {
        let more = rows.len() > self.size as usize;
        rows.truncate(self.size as usize);
        let next_token = match rows.last() {
            Some(last) if more => {
                let (key, id) = place(last);
                let cursor = Cursor {
                    key,
                    id,
                    list_hash: self.list_hash,
                };
                URL_SAFE_NO_PAD.encode(serde_json::to_vec(&cursor).expect("serializes"))
            }
            _ => String::new(),
        };
        PageResponse {
            next_token,
            ..Default::default()
        }
    }
}

#[expect(
    clippy::expect_used,
    reason = "a SHA-256 digest is longer than the hash taken from it"
)]
fn hash(list: &[u8]) -> Hash {
    *Sha256::digest(list).first_chunk().expect("32 bytes")
}

#[cfg(test)]
#[path = "page_tests.rs"]
mod tests;
