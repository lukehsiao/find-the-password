//! Helpers shared across the test modules: the in-process HTTP harness and
//! the generators for the domain's inputs.

use hegel::{
    extras::jiff as jiff_gs,
    generators::{self, PrintableGenerator},
};
use jiff::Timestamp;
use topcoat::{
    context::BaseUrl,
    router::{
        HeaderMap, Method, Router, StatusCode, header,
        request::{Bytes, Request},
        to_bytes,
    },
};

use crate::{clock::Clock, router, store::ChallengeStore};

/// Usernames the challenge accepts.
pub(crate) fn usernames() -> impl PrintableGenerator<String> {
    generators::from_regex(r"[a-zA-Z0-9._-]{3,32}").fullmatch(true)
}

/// Instants bound to 2000..2090, so the millisecond seed math and the
/// solve-time offsets tests add can never overflow; the logic doesn't care
/// about the era.
pub(crate) fn timestamps() -> impl PrintableGenerator<Timestamp> {
    let min = Timestamp::from_second(946_684_800).unwrap();
    let max = Timestamp::from_second(3_786_825_600).unwrap();
    jiff_gs::timestamps().min_value(min).max_value(max)
}

/// A guaranteed-wrong guess: longer than any 32-character secret.
pub(crate) fn wrong_guess(secret: &str) -> String {
    format!("{secret}-nope")
}

/// The base URL the test router serves players at. A host no deployment
/// uses, so an assertion on it cannot pass by accident.
pub(crate) const BASE_URL: &str = "https://find-the-password.test";

/// The production router over a fresh store and a nulled clock, driven
/// without a socket.
///
/// Tests seed and inspect state through `store`, move time with `clock`,
/// and exercise it all over HTTP through the request helpers.
pub(crate) struct TestApp {
    router: Router,
    pub(crate) store: ChallengeStore,
    pub(crate) clock: Clock,
}

/// A buffered response.
pub(crate) struct Reply {
    pub(crate) status: StatusCode,
    pub(crate) headers: HeaderMap,
    pub(crate) body: String,
}

impl Reply {
    /// The value of header `name`, which must be present and ASCII.
    pub(crate) fn header(&self, name: &header::HeaderName) -> &str {
        self.headers
            .get(name)
            .unwrap_or_else(|| panic!("missing {name} header"))
            .to_str()
            .unwrap()
    }
}

impl TestApp {
    pub(crate) fn new() -> Self {
        let store = ChallengeStore::new();
        let clock = Clock::null();
        Self {
            router: router(
                store.clone(),
                clock.clone(),
                BaseUrl::new(BASE_URL).unwrap(),
            ),
            store,
            clock,
        }
    }

    pub(crate) async fn get(&self, uri: &str) -> Reply {
        self.send(Request::builder().uri(uri).body(().into()).unwrap())
            .await
    }

    /// POST `form` URL-encoded with no fetch-metadata headers, the way a
    /// player's script would; the origin policy lets those through.
    pub(crate) async fn post_form(&self, uri: &str, form: &str) -> Reply {
        let request = Request::builder()
            .method(Method::POST)
            .uri(uri)
            .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
            .body(form.to_owned().into())
            .unwrap();
        self.send(request).await
    }

    /// GET `uri` accepting `encoding`, returning the headers and the body
    /// bytes exactly as they would go out on the wire.
    pub(crate) async fn get_encoded(&self, uri: &str, encoding: &str) -> (HeaderMap, Bytes) {
        let request = Request::builder()
            .uri(uri)
            .header(header::ACCEPT_ENCODING, encoding)
            .body(().into())
            .unwrap();
        let (_, headers, body) = self.send_raw(request).await;
        (headers, body)
    }

    async fn send(&self, request: Request) -> Reply {
        let (status, headers, body) = self.send_raw(request).await;
        Reply {
            status,
            headers,
            body: String::from_utf8(body.to_vec()).unwrap(),
        }
    }

    async fn send_raw(&self, request: Request) -> (StatusCode, HeaderMap, Bytes) {
        let (parts, body) = self.router.handle(request).await.into_parts();
        let body = to_bytes(body, usize::MAX).await.unwrap();
        (parts.status, parts.headers, body)
    }
}

/// `segment` percent-encoded to sit in one path segment.
///
/// Every byte but ASCII alphanumerics is encoded, which RFC 3986 allows for
/// any character. Encoding the dots too keeps `.` and `..` from reading as
/// dot-segments, which URL libraries would otherwise resolve away before
/// the request is sent.
pub(crate) fn path_segment(segment: &str) -> String {
    segment
        .bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() {
                char::from(b).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect()
}

/// `pairs` encoded as an `application/x-www-form-urlencoded` body, the way
/// a browser submits a form.
pub(crate) fn form<'a>(pairs: impl IntoIterator<Item = (&'a str, &'a str)>) -> String {
    url::form_urlencoded::Serializer::new(String::new())
        .extend_pairs(pairs)
        .finish()
}
