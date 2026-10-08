//! The plain-text routes that players' scripts depend on, plus the
//! operational endpoints.

use topcoat::{
    Result,
    context::Cx,
    router::{
        HeaderName, HeaderValue, StatusCode,
        error::{RouterErrorExt, not_found},
        header::CONTENT_TYPE,
        path_param, route,
    },
};

use crate::{Username, store, store::CheckOutcome};

path_param!(password);

/// Simple healthcheck endpoint.
#[route(GET "/up")]
pub async fn healthcheck() -> Result<StatusCode> {
    Ok(StatusCode::OK)
}

/// Tell every crawler to stay away from every page.
///
/// The site is a classroom game: per-player check URLs and passwords.txt
/// downloads are noise no search index should surface. A `&'static str`
/// response already carries `text/plain; charset=utf-8`, which is what
/// robots.txt requires.
#[route(GET "/robots.txt")]
pub async fn robots_txt() -> Result<&'static str> {
    Ok("User-agent: *\nDisallow: /\n")
}

/// The site icon, compiled into the binary so the server is one file.
#[route(GET "/favicon.ico")]
pub async fn favicon() -> Result<([(HeaderName, HeaderValue); 1], &'static [u8])> {
    Ok((
        [(CONTENT_TYPE, HeaderValue::from_static("image/x-icon"))],
        include_bytes!("../public/favicon.ico"),
    ))
}

/// Check a password for correctness.
///
/// The literal `true`/`false` bodies and the 200/404 statuses are the
/// contract that players' scripts depend on.
#[route(GET "/u/{username}/check/{password}")]
pub async fn check_password(cx: &Cx) -> Result<&'static str> {
    // Unparsed path parameters borrow the router's decoded captures, so the
    // hottest route in the app allocates nothing of its own.
    match store(cx).check(path_param::<Username>(cx), path_param::<Password>(cx)) {
        CheckOutcome::NotFound => Err(not_found().into()),
        CheckOutcome::Incorrect => Ok("false"),
        CheckOutcome::Correct => Ok("true"),
    }
}

/// Produce passwords.txt for a user.
#[route(GET "/u/{username}/passwords.txt")]
pub async fn passwords_txt(cx: &Cx) -> Result<String> {
    Ok(store(cx)
        .passwords(path_param::<Username>(cx))
        .ok_or_not_found()?)
}

// The kid-facing HTTP contract, driven through the production router. These
// lock the exact responses players' scripts depend on: literal `true`/`false`
// bodies, 200/404 statuses, and a byte-for-byte passwords.txt. In-crate so
// they can read the crate-private `User::secret` as the correct-password
// oracle.
#[cfg(test)]
mod tests {
    use std::io::Read;

    use flate2::read::GzDecoder;
    use hegel::generators;
    use topcoat::router::{StatusCode, header};

    use crate::testing::{TestApp, path_segment};

    #[tokio::test]
    async fn healthcheck_returns_200() {
        let app = TestApp::new();
        assert_eq!(app.get("/up").await.status, StatusCode::OK);
    }

    #[tokio::test]
    async fn robots_txt_disallows_all_crawlers() {
        let reply = TestApp::new().get("/robots.txt").await;
        assert_eq!(reply.status, StatusCode::OK);
        assert_eq!(
            reply.header(&header::CONTENT_TYPE),
            "text/plain; charset=utf-8"
        );
        assert_eq!(reply.body, "User-agent: *\nDisallow: /\n");
    }

    #[tokio::test]
    async fn check_for_unknown_user_is_404() {
        let reply = TestApp::new().get("/u/ghost/check/whatever").await;
        assert_eq!(reply.status, StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn wrong_password_returns_false() {
        let app = TestApp::new();
        app.store.add_user("alice", app.clock.now()).unwrap();
        let reply = app.get("/u/alice/check/definitely-wrong").await;
        assert_eq!(reply.status, StatusCode::OK);
        assert_eq!(reply.body, "false");
    }

    #[tokio::test]
    async fn correct_password_returns_true() {
        let app = TestApp::new();
        app.store.add_user("bob", app.clock.now()).unwrap();
        let secret = app.store.get_user("bob").unwrap().secret;
        let reply = app.get(&format!("/u/bob/check/{secret}")).await;
        assert_eq!(reply.status, StatusCode::OK);
        assert_eq!(reply.body, "true");
    }

    // Locks the confirm-flow contract: the check URL reads true but never
    // solves, no matter how often the correct password goes past.
    #[tokio::test]
    async fn correct_check_does_not_solve() {
        let app = TestApp::new();
        app.store.add_user("dave", app.clock.now()).unwrap();
        let secret = app.store.get_user("dave").unwrap().secret;

        for _ in 0..2 {
            let reply = app.get(&format!("/u/dave/check/{secret}")).await;
            assert_eq!(reply.status, StatusCode::OK);
            assert_eq!(reply.body, "true");
        }

        assert!(app.store.get_user("dave").unwrap().solved_at.is_none());
        assert!(app.store.leaders().is_empty());
    }

    #[tokio::test]
    async fn passwords_download_matches_store() {
        let app = TestApp::new();
        app.store.add_user("carol", app.clock.now()).unwrap();
        let expected = app.store.passwords("carol").unwrap();
        let reply = app.get("/u/carol/passwords.txt").await;
        assert_eq!(reply.status, StatusCode::OK);
        assert_eq!(
            reply.header(&header::CONTENT_TYPE),
            "text/plain; charset=utf-8"
        );
        assert_eq!(reply.body, expected);
        assert_eq!(reply.body.lines().count(), 60_000);
    }

    // The file is 2MB and players download it over school wifi, so it must
    // go out compressed to any client that asks, and decode to the same
    // bytes.
    #[tokio::test]
    async fn passwords_download_is_gzipped_when_accepted() {
        let app = TestApp::new();
        app.store.add_user("carol", app.clock.now()).unwrap();
        let (headers, body) = app.get_encoded("/u/carol/passwords.txt", "gzip").await;
        assert_eq!(headers[header::CONTENT_ENCODING], "gzip");

        let mut decoded = String::new();
        GzDecoder::new(&body[..])
            .read_to_string(&mut decoded)
            .unwrap();
        assert_eq!(decoded, app.store.passwords("carol").unwrap());
        assert!(body.len() < decoded.len());
    }

    #[tokio::test]
    async fn passwords_for_unknown_user_is_404() {
        let reply = TestApp::new().get("/u/ghost/passwords.txt").await;
        assert_eq!(reply.status, StatusCode::NOT_FOUND);
    }

    // Whatever a script puts in the password segment, percent-encoded as
    // any HTTP client would, reaches the store decoded and intact: only the
    // secret itself reads true, and every guess counts once.
    #[tokio::test]
    #[hegel::test]
    async fn check_answers_true_exactly_for_the_secret(tc: hegel::TestCase) {
        let app = TestApp::new();
        app.store.add_user("alice", app.clock.now()).unwrap();
        let secret = app.store.get_user("alice").unwrap().secret;
        let guess = if tc.draw(generators::booleans()) {
            secret.clone()
        } else {
            tc.draw(generators::text().min_size(1))
        };

        let reply = app
            .get(&format!("/u/alice/check/{}", path_segment(&guess)))
            .await;
        assert_eq!(reply.status, StatusCode::OK);
        assert_eq!(reply.body, if guess == secret { "true" } else { "false" });
        assert_eq!(app.store.get_user("alice").unwrap().hits_before_solved, 1);
    }
}
