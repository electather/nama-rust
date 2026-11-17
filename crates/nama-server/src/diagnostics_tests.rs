use super::service_and_method;

#[test]
fn service_and_method_names_the_service_and_method_of_a_connect_path() {
    assert_eq!(
        service_and_method("/nama.v1.SignInService/SignIn"),
        Some(("nama.v1.SignInService", "SignIn"))
    );
}

#[test]
fn service_and_method_is_none_for_a_path_that_is_no_rpc() {
    for path in [
        "/",
        "/index.html",
        "/media/12/master.m3u8",
        "/custom-images/plugin/tmdb",
        "/nama.v1.SignInService/SignIn/more",
    ] {
        assert_eq!(service_and_method(path), None, "{path}");
    }
}
