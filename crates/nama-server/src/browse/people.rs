//! A Person's page (`GLOSSARY.md`): the titles in a Library they have a Role in.

use nama_plugin_api::metadata::ExternalId;

use super::{Browse, Error};
use crate::library::{ImageRef, NotFound, OnServer, PersonRole, TitleKind};

/// How many People a search finds at most.
const PEOPLE_FOUND: usize = 5;

/// How many People whose names fit a search it reads the pages of, to find [`PEOPLE_FOUND`] with
/// a title in a Library.
// ponytail: a Person ranked below these is missed when those above have no title in a Library;
// raise it, or keep only Roles of titles on the Server, if one ever is.
const PEOPLE_READ: usize = 20;

/// A Person as their page shows them.
#[derive(Debug)]
pub(crate) struct PersonPage {
    pub(crate) id: ExternalId,
    /// As their first Role names them.
    pub(crate) name: String,
    /// The first headshot their Roles have.
    pub(crate) headshot: Option<ImageRef>,
    /// The titles in a Library they play a character in, newest first.
    pub(crate) cast: Vec<PersonTitle>,
    /// The titles in a Library they did a job on, newest first.
    pub(crate) crew: Vec<PersonTitle>,
}

/// A title in a Library, and a Person's parts in it in billing order, empty ones left out.
#[derive(Debug)]
pub(crate) struct PersonTitle {
    pub(crate) title: OnServer,
    pub(crate) parts: Vec<String>,
}

impl Browse {
    /// The page of `person`; [`NotFound::NoSuchPerson`] if no title in a Library has a Role of
    /// theirs.
    pub(crate) async fn person(&self, person: &ExternalId) -> Result<PersonPage, Error> {
        let roles = self.contents.person_roles(person).await?;
        let (crew, cast): (Vec<&PersonRole>, _) = roles.iter().partition(|role| role.crew);
        let cast = self.titles_of(&cast).await?;
        let crew = self.titles_of(&crew).await?;
        let first = roles
            .first()
            .filter(|_| !(cast.is_empty() && crew.is_empty()));
        let first = first.ok_or(NotFound::NoSuchPerson)?;
        let headshot = roles.iter().find_map(|role| {
            Some(ImageRef {
                plugin: role.plugin.clone().into(),
                path: role.role.headshot.clone()?,
            })
        });
        Ok(PersonPage {
            id: person.clone(),
            name: first.role.name.clone(),
            headshot,
            cast,
            crew,
        })
    }

    /// The pages of at most [`PEOPLE_FOUND`] People with a title in a Library whose names fit
    /// `query`, best first, as [`Contents::people_named`](crate::library::Contents) orders them.
    pub(crate) async fn people_named(&self, query: &str) -> Result<Vec<PersonPage>, Error> {
        let mut found = Vec::new();
        for person in (self.contents.people_named(query).await?)
            .iter()
            .take(PEOPLE_READ)
        {
            match self.person(person).await {
                Ok(page) => found.push(page),
                Err(Error::NotFound(NotFound::NoSuchPerson)) => continue,
                Err(err) => return Err(err),
            }
            if found.len() == PEOPLE_FOUND {
                break;
            }
        }
        Ok(found)
    }

    /// The titles in a Library that `roles` are in, each once with its parts, newest first, those
    /// without a date last.
    async fn titles_of(&self, roles: &[&PersonRole]) -> Result<Vec<PersonTitle>, Error> {
        let mut titles = Vec::new();
        for kind in [TitleKind::Movie, TitleKind::Show] {
            let of_kind: Vec<&PersonRole> = roles
                .iter()
                .copied()
                .filter(|role| role.kind == kind)
                .collect();
            let ids: Vec<ExternalId> = of_kind.iter().map(|role| role.title.clone()).collect();
            for title in self.in_libraries(kind, &ids).await? {
                let parts = (of_kind.iter())
                    .filter(|role| title.ids().iter().any(|id| id == role.title))
                    .filter(|role| !role.role.part.is_empty())
                    .map(|role| role.role.part.clone())
                    .collect();
                titles.push(PersonTitle { title, parts });
            }
        }
        titles.sort_by(|a, b| b.title.date().cmp(&a.title.date()));
        Ok(titles)
    }
}

#[cfg(test)]
#[path = "people_tests.rs"]
mod tests;
