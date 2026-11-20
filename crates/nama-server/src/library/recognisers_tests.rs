use std::sync::{Arc, Mutex, PoisonError};

use nama_plugin_api::{
    BoxFuture, Manifest, Plugin, PluginError, PluginId, PointKey, Priority, Registrar,
    contract::{FileRecognised, FileToRecognise, TitleRecognised, TitleToRecognise},
    recogniser::{Recogniser, RecogniserMethod},
};
use tempfile::TempDir;
use tokio::sync::Notify;

use super::{RecogniserFailed, Recognisers};
use crate::{
    db::Database,
    library::{Change, Contents, Decided, Item, Libraries, PARSER_VERSION, fixture},
    log_capture::CapturedLog,
    plugins::Registry,
};

/// The ids of the Recognisers asked, in the order asked.
type Asked = Arc<Mutex<Vec<&'static str>>>;

/// The Plugin `id`'s Recogniser, serving `serves` and failing every call when `fails`, with what
/// it was asked about in its error.
#[derive(Clone)]
struct Fake {
    id: &'static str,
    serves: &'static [RecogniserMethod],
    fails: bool,
    asked: Asked,
}

impl Fake {
    fn answer<T: Default + Send + 'static>(
        &self,
        about: &str,
    ) -> BoxFuture<'_, Result<T, PluginError>> {
        self.asked
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(self.id);
        let answer = if self.fails {
            Err(format!("down on {about}").into())
        } else {
            Ok(T::default())
        };
        Box::pin(std::future::ready(answer))
    }
}

impl Recogniser for Fake {
    fn recognise_title<'a>(
        &'a self,
        title: &'a TitleToRecognise,
    ) -> BoxFuture<'a, Result<TitleRecognised, PluginError>> {
        self.answer(&title.title)
    }

    fn recognise_file<'a>(
        &'a self,
        file: &'a FileToRecognise,
    ) -> BoxFuture<'a, Result<FileRecognised, PluginError>> {
        self.answer(&file.path)
    }

    fn recognises(&self, method: RecogniserMethod) -> bool {
        self.serves.contains(&method)
    }
}

impl Plugin for Fake {
    fn manifest(&self) -> Manifest {
        Manifest {
            version: "0.0.0".to_owned(),
            extension_points: vec![PointKey::provide::<dyn Recogniser>()],
            ..Manifest::new(PluginId::from_static(self.id))
        }
    }

    fn register(&self, registrar: &mut Registrar) -> Result<(), PluginError> {
        registrar.provide::<dyn Recogniser>(Priority::DEFAULT, Arc::new(self.clone()));
        Ok(())
    }
}

const BOTH: &[RecogniserMethod] = &[RecogniserMethod::File, RecogniserMethod::Title];

/// A Registry with `fakes` enabled, and what they are asked.
fn enabled(fakes: &[(&'static str, &'static [RecogniserMethod], bool)]) -> (Registry, Asked) {
    let plugins = Registry::new();
    let asked = Asked::default();
    for &(id, serves, fails) in fakes {
        let asked = Arc::clone(&asked);
        let fake = Fake {
            id,
            serves,
            fails,
            asked,
        };
        plugins.enable(Arc::new(fake)).unwrap();
    }
    (plugins, asked)
}

fn recognisers(plugins: &Registry, level: RecogniserMethod) -> Recognisers {
    Recognisers::new(plugins.extension_points(), level)
}

fn order(ids: &[&str]) -> Vec<String> {
    ids.iter().map(|&id| id.to_owned()).collect()
}

/// What `recognisers` answer about the file at `path`, asking each it lists in turn.
async fn ask_file(
    recognisers: &Recognisers,
    path: &str,
) -> Vec<Result<Option<FileRecognised>, RecogniserFailed>> {
    let file = &FileToRecognise {
        path: path.to_owned(),
        ..FileToRecognise::default()
    };
    let mut answers = Vec::new();
    for plugin in recognisers.listed() {
        let answer = recognisers
            .ask(plugin, |recogniser: Arc<dyn Recogniser>| async move {
                recogniser.recognise_file(file).await
            })
            .await;
        answers.push(answer);
    }
    answers
}

fn asked(asked: &Asked) -> Vec<&'static str> {
    asked.lock().unwrap_or_else(PoisonError::into_inner).clone()
}

#[tokio::test]
async fn in_order_keeps_the_librarys_order_of_enabled_recognisers_only() {
    let (plugins, asked_of) = enabled(&[("aa", BOTH, false), ("bb", BOTH, false)]);
    let recognisers = recognisers(&plugins, RecogniserMethod::File);

    let listed = recognisers.in_order(&order(&["gone", "bb", "aa"]));
    ask_file(&listed, "Show - 1.mkv").await;

    assert_eq!(
        listed.listed(),
        [PluginId::from_static("bb"), PluginId::from_static("aa")]
    );
    assert_eq!(asked(&asked_of), ["bb", "aa"]);
}

#[tokio::test]
async fn a_recogniser_disabled_since_it_was_listed_is_skipped() {
    let (plugins, asked_of) = enabled(&[("aa", BOTH, false)]);
    let recognisers = recognisers(&plugins, RecogniserMethod::File);
    let listed = recognisers.in_order(&order(&["aa"]));

    plugins.disable_now(&PluginId::from_static("aa"));
    let answers = ask_file(&listed, "Show - 1.mkv").await;

    assert!(matches!(answers[..], [Ok(None)]), "{answers:?}");
    assert_eq!(asked(&asked_of), Vec::<&str>::new());
}

#[tokio::test]
async fn a_recogniser_not_serving_the_levels_method_is_skipped() {
    let titles_only: &[RecogniserMethod] = &[RecogniserMethod::Title];
    let (plugins, asked_of) = enabled(&[("aa", titles_only, false), ("bb", BOTH, false)]);
    let recognisers = recognisers(&plugins, RecogniserMethod::File);

    let answers = ask_file(&recognisers.in_order(&order(&["aa", "bb"])), "Show - 1.mkv").await;

    assert!(
        matches!(answers[..], [Ok(None), Ok(Some(_))]),
        "{answers:?}"
    );
    assert_eq!(asked(&asked_of), ["bb"]);
}

#[tokio::test]
async fn a_failure_is_counted_and_logged_once_with_its_count_and_last_error() {
    let (plugins, _) = enabled(&[("aa", BOTH, true), ("bb", BOTH, false)]);
    let recognisers = recognisers(&plugins, RecogniserMethod::File);
    let listed = recognisers.in_order(&order(&["aa", "bb"]));
    // The test's runtime has one thread, so the log is written to it.
    let log = CapturedLog::start();

    let first = ask_file(&listed, "Show - 1.mkv").await;
    ask_file(&listed, "Show - 2.mkv").await;
    drop(listed);

    let log = log.finish();
    assert!(matches!(first[..], [Err(RecogniserFailed), Ok(Some(_))]));
    let warned: Vec<&str> = log.lines().filter(|line| line.contains("WARN")).collect();
    assert_eq!(warned.len(), 1, "{log}");
    assert!(
        warned[0].contains(
            "a Recogniser failed, so the files it failed on are read again on the next scan"
        ),
        "{log}"
    );
    assert!(warned[0].contains("plugin=aa files=2"), "{log}");
    assert!(
        warned[0].contains(r#"last_error="down on Show - 2.mkv""#),
        "{log}"
    );
}

#[tokio::test]
async fn failures_are_logged_when_the_scan_lets_go_however_it_ends() {
    let (plugins, _) = enabled(&[("aa", BOTH, true)]);
    let recognisers = recognisers(&plugins, RecogniserMethod::Title);
    let listed = recognisers.in_order(&order(&["aa"]));
    let held = listed.clone();
    let title = &TitleToRecognise {
        title: "Dark".to_owned(),
        ..TitleToRecognise::default()
    };
    let held_on = CapturedLog::start();

    let failed = listed
        .ask(&PluginId::from_static("aa"), |recogniser: Arc<dyn Recogniser>| async move {
            recogniser.recognise_title(title).await
        })
        .await;
    // A scan that ends, cancelled or not, drops its Recognisers; a clone still held keeps the
    // tally open.
    drop(listed);
    let while_held = held_on.finish();
    let letting_go = CapturedLog::start();
    drop(held);

    let logged = letting_go.finish();
    assert!(matches!(failed, Err(RecogniserFailed)));
    assert!(!while_held.contains("WARN"), "{while_held}");
    assert!(
        logged.contains(
            "a Recogniser failed, so the titles it failed on are matched again on the next scan"
        ),
        "{logged}"
    );
    assert!(logged.contains("plugin=aa titles=1"), "{logged}");
}

#[tokio::test]
async fn failures_are_logged_when_a_scan_is_cancelled_while_asking() {
    let (plugins, _) = enabled(&[("aa", BOTH, true)]);
    let listed = recognisers(&plugins, RecogniserMethod::File).in_order(&order(&["aa"]));
    let asking = Arc::new(Notify::new());
    let asked_again = Arc::clone(&asking);
    // The test's runtime has one thread, so the aborted scan logs to it.
    let log = CapturedLog::start();

    let scan = tokio::spawn(async move {
        ask_file(&listed, "Show - 1.mkv").await;
        let aa = PluginId::from_static("aa");
        let answer = listed.ask(&aa, |_: Arc<dyn Recogniser>| {
            asked_again.notify_one();
            std::future::pending::<Result<FileRecognised, PluginError>>()
        });
        answer.await
    });
    asking.notified().await;
    scan.abort();
    let ended = scan.await;

    let logged = log.finish();
    assert!(ended.is_err_and(|ended| ended.is_cancelled()));
    assert!(logged.contains("plugin=aa files=1"), "{logged}");
}

/// Stores an unparsed file and a Movie that Matching left Unmatched in the Library `id`.
async fn unparsed_and_unmatched(contents: &Contents, id: i64) {
    let store = |path: &str, items| Change::Store {
        path: path.to_owned(),
        size: 1,
        modified_ns: 1,
        items,
    };
    let dark = Item::Movie {
        title: "Dark".to_owned(),
        year: None,
        hint: None,
    };
    let files = vec![store(UNPARSED, Vec::new()), store(UNMATCHED, vec![dark])];
    contents.apply(id, files).await.unwrap();
    let unmatched = vec![Decided::Unmatched(None)];
    fixture::decide(contents, id, UNMATCHED, unmatched).await;
}

const UNPARSED: &str = "/media/Show - 2.mkv";
const UNMATCHED: &str = "/media/Dark.mkv";

/// The unparsed file's parser version, and whether the Unmatched decision is still stored.
async fn read_again(contents: &Contents, id: i64) -> (i64, bool) {
    let parser_version = contents.stored(id).await.unwrap()[UNPARSED].parser_version;
    let files = contents.files(id).await.unwrap();
    let dark = files.iter().find(|file| file.path == UNMATCHED).unwrap();
    (parser_version, dark.items[0].recorded.is_some())
}

#[tokio::test]
async fn a_changed_listing_reads_files_again_in_one_write_and_an_unchanged_one_doesnt() {
    let (plugins, _) = enabled(&[("aa", BOTH, false)]);
    let (data, media) = (TempDir::new().unwrap(), TempDir::new().unwrap());
    let db = Arc::new(Database::open(data.path()).await.unwrap());
    let recognisers = recognisers(&plugins, RecogniserMethod::File);
    let (libraries, contents) = (
        Libraries::new(Arc::clone(&db)),
        Contents::new(Arc::clone(&db)),
    );
    let id = fixture::movies_library(&db, media.path()).await;
    libraries.set_recognisers(id, order(&["aa"])).await.unwrap();
    let library = libraries.get(id).await.unwrap();

    unparsed_and_unmatched(&contents, id).await;
    recognisers
        .read_again_if_changed(&libraries, &library)
        .await
        .unwrap();
    let changed = read_again(&contents, id).await;
    unparsed_and_unmatched(&contents, id).await;
    recognisers
        .read_again_if_changed(&libraries, &library)
        .await
        .unwrap();
    let unchanged = read_again(&contents, id).await;

    assert_eq!(changed, (PARSER_VERSION - 1, false));
    assert_eq!(unchanged, (PARSER_VERSION, true));
}
