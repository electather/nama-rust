//! What the lasting steps allow past the plan they shaped: the choices it can ever honour, and,
//! when it can't be played, the other Versions of its title that would play.

use std::collections::BTreeSet;

use super::{
    Facts, LASTING_STEPS, Method, Offers, Probe, Probing, Track, candidates,
    choices::burns_in,
    codecs::{file_audio, is_audio},
    limits::{fitted, picture},
    narrowed,
};
use crate::server_settings::PlaybackLimit;

/// What a plan for `facts` can ever honour, with only the methods in `lasting` allowed.
pub(super) fn offers(facts: &Facts, lasting: &BTreeSet<Method>) -> Offers {
    let Some(probe) = &facts.probe else {
        return Offers::default();
    };
    let transcodes = lasting.contains(&Method::Transcode);
    let places = |keep: &dyn Fn(&Track) -> bool| {
        (probe.tracks.iter().enumerate())
            .filter(|(_, track)| keep(track))
            .map(|(place, _)| place)
            .collect()
    };
    // Only the file's own audio plays without a Remux or a Transcode.
    let audio = if transcodes || lasting.contains(&Method::Remux) {
        places(&is_audio)
    } else {
        file_audio(probe).into_iter().collect()
    };
    if !transcodes {
        return Offers {
            audio,
            ..Offers::default()
        };
    }
    let picture = picture(facts);
    Offers {
        audio,
        burn_in: places(&|track| burns_in(&facts.device, probe, track)),
        limits: (PlaybackLimit::ALL.into_iter())
            .filter(|&limit| picture.and_then(|picture| fitted(picture, limit)).is_some())
            .collect(),
    }
}

/// The other Versions of the title `facts.version` is of that the lasting steps let play on its
/// Device and this Server, by id, best method first, then by path. Only Tracks already stored are
/// read: a Version not probed yet is left out, so an unplayable plan never waits on ffprobe.
pub(super) async fn playable_versions(
    probing: &Probing,
    facts: &Facts,
) -> Result<Vec<i64>, sqlx::Error> {
    let mut playable: Vec<(Method, i64)> = (probing.stored_other_versions(facts.version).await?)
        .into_iter()
        .filter_map(|(version, probe)| Some((best_lasting_method(facts, version, probe)?, version)))
        .collect();
    // A stable sort, so those of one method stay in path order.
    playable.sort_by_key(|&(method, _)| method);
    Ok(playable.into_iter().map(|(_, version)| version).collect())
}

/// The best method the lasting steps allow for the Version `version`, probed as `probe`, on
/// `facts`' Device and Server, if any.
fn best_lasting_method(facts: &Facts, version: i64, probe: Probe) -> Option<Method> {
    // The Tracks asked for are places in the asked Version's Tracks, not this one's.
    let facts = Facts {
        version,
        probe: Some(probe),
        audio: None,
        subtitle: None,
        ..facts.clone()
    };
    let all = BTreeSet::from(Method::ALL);
    let lasting = narrowed(
        &facts,
        candidates(&facts).as_ref(),
        &LASTING_STEPS,
        all,
        &mut vec![],
    );
    lasting.first().copied()
}
