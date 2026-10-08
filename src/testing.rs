//! In-process HTTP harness shared by the route and page tests.

use topcoat::router::{HeaderMap, Method, Router, StatusCode, header, request::Request, to_bytes};

use crate::{clock::Clock, router, store::ChallengeStore};

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
            router: router(store.clone(), clock.clone()),
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

    async fn send(&self, request: Request) -> Reply {
        let response = self.router.handle(request).await;
        let (parts, body) = response.into_parts();
        let body = to_bytes(body, usize::MAX).await.unwrap();
        Reply {
            status: parts.status,
            headers: parts.headers,
            body: String::from_utf8(body.to_vec()).unwrap(),
        }
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
