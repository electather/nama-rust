use nama_plugin_api::server_link::Reachability;
use rcgen::{CertificateParams, KeyPair};

use super::{
    ACCOUNT_FILE, CERTIFICATE_FILE, Certificate, Error, RENEW_BEFORE_SECS, account, parse,
    public_url,
};

const OWN: &str = "abc.namadirect.test";

/// 2030-01-01, when [`certificate_pem`]'s certificates expire, in seconds since the Unix epoch.
const EXPIRES_AT: i64 = 1_893_456_000;

const DAY_SECS: i64 = 24 * 60 * 60;

/// A certificate for [`OWN`] and the wildcard under it, as an ACME CA gives one, with its key
/// first, as the Server keeps it.
fn certificate_pem() -> String {
    let mut params = CertificateParams::new([OWN.to_owned(), format!("*.{OWN}")]).unwrap();
    params.not_after = rcgen::date_time_ymd(2030, 1, 1);
    let key = KeyPair::generate().unwrap();
    let certificate = params.self_signed(&key).unwrap();
    format!("{}{}", key.serialize_pem(), certificate.pem())
}

fn held() -> Certificate {
    let certificate = Certificate::default();
    certificate.hold(parse(certificate_pem().as_bytes()).unwrap());
    certificate
}

fn reached(address: &str, reachable: bool) -> Reachability {
    Reachability {
        address: address.to_owned(),
        reachable,
        names: vec![OWN.to_owned(), format!("192-168-1-10.{OWN}")],
    }
}

#[test]
fn a_kept_certificate_reads_as_the_names_it_is_for_and_when_it_expires() {
    let held = parse(certificate_pem().as_bytes()).unwrap();

    assert_eq!(held.names, [OWN.to_owned(), format!("*.{OWN}")]);
    assert_eq!(held.expires_at, EXPIRES_AT);
}

#[test]
fn a_file_without_a_key_isnt_a_certificate() {
    let without_key = certificate_pem()
        .split_once("-----END PRIVATE KEY-----\n")
        .unwrap()
        .1
        .to_owned();

    parse(without_key.as_bytes()).unwrap_err();
}

#[test]
fn a_certificate_is_renewed_within_a_month_of_expiring_or_once_the_servers_name_changes() {
    let certificate = held();

    let five_weeks_before = EXPIRES_AT - 35 * DAY_SECS;
    let three_weeks_before = EXPIRES_AT - 21 * DAY_SECS;
    assert!(certificate.lasts(OWN, five_weeks_before, RENEW_BEFORE_SECS));
    assert!(!certificate.lasts(OWN, three_weeks_before, RENEW_BEFORE_SECS));
    assert!(!certificate.lasts("def.namadirect.test", five_weeks_before, RENEW_BEFORE_SECS));
    assert!(!Certificate::default().lasts(OWN, five_weeks_before, 0));
}

#[test]
fn the_servers_public_name_on_its_public_port_is_its_default_url_while_the_hub_reaches_it() {
    let certificate = held();
    let now = EXPIRES_AT - DAY_SECS;

    assert_eq!(
        public_url(&reached("http://203.0.113.7:7980", true), &certificate, now).as_deref(),
        Some("https://abc.namadirect.test:7980")
    );
    assert_eq!(
        public_url(&reached("http://203.0.113.7", true), &certificate, now).as_deref(),
        Some("https://abc.namadirect.test:80")
    );
    assert_eq!(
        public_url(
            &reached("http://203.0.113.7:7980", false),
            &certificate,
            now
        ),
        None
    );
    assert_eq!(public_url(&reached("", false), &certificate, now), None);
}

#[test]
fn without_a_certificate_for_its_name_the_server_has_no_default_url() {
    let reachable = reached("http://203.0.113.7:7980", true);

    assert_eq!(public_url(&reachable, &Certificate::default(), 0), None);
    assert_eq!(public_url(&reachable, &held(), EXPIRES_AT), None);
}

#[tokio::test]
async fn the_certificate_kept_in_the_data_directory_is_served() {
    let data_dir = tempfile::tempdir().unwrap();
    std::fs::write(data_dir.path().join(CERTIFICATE_FILE), certificate_pem()).unwrap();
    let certificate = Certificate::default();

    certificate.load(data_dir.path()).await;

    assert!(certificate.lasts(OWN, EXPIRES_AT - DAY_SECS, 0));
}

#[tokio::test]
async fn a_kept_acme_account_that_cant_be_read_is_made_again() {
    let data_dir = tempfile::tempdir().unwrap();
    std::fs::write(data_dir.path().join(ACCOUNT_FILE), "{").unwrap();

    // Nothing listens there, so making the account fails at the CA, past reading the kept one.
    let made = account(data_dir.path(), "https://127.0.0.1:1/").await;

    assert!(matches!(made, Err(Error::Acme(_))), "{:?}", made.err());
}
