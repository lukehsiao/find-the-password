use jiff::{Span, SpanRound, Unit};
use leptos::{either::Either, prelude::*};
use leptos_meta::{MetaTags, Stylesheet, Title, provide_meta_context};
use leptos_router::{
    components::{Redirect, Route, Router, Routes},
    hooks::use_params,
    params::Params,
    path,
};

use crate::{
    error::AppError,
    user::{Completion, RosterEntry, User},
};

/// Attach an error's semantically correct HTTP status to the response.
///
/// `server_fn` answers every `Err` with a blanket 500; overriding through
/// [`leptos_axum::ResponseOptions`] is the idiomatic escape hatch. Plain
/// HTML form posts (JS disabled) are excluded, exactly as
/// `leptos_axum::redirect` excludes them: those are answered with a 302
/// back to the referring page, which a status override would stomp.
#[cfg(feature = "ssr")]
fn with_status(error: AppError) -> AppError {
    use axum::http::{HeaderValue, header, request::Parts};
    use leptos_axum::ResponseOptions;

    let accepts_html = use_context::<Parts>()
        .and_then(|parts| {
            parts
                .headers
                .get(header::ACCEPT)
                .and_then(|accept| accept.to_str().ok())
                .map(|accept| accept.contains("text/html"))
        })
        .unwrap_or(false);
    if accepts_html {
        return error;
    }

    if let Some(response) = use_context::<ResponseOptions>() {
        response.set_status(error.status());
        if matches!(error, AppError::ConfirmThrottled) {
            // The upper bound on the wait; the exact remainder is not
            // worth threading through the domain types.
            let secs = crate::user::CONFIRM_COOLDOWN.as_secs().to_string();
            response.insert_header(
                header::RETRY_AFTER,
                HeaderValue::from_str(&secs).expect("whole seconds are a valid header value"),
            );
        }
    }
    error
}

/// Add a new user to the user map.
// allow, not expect: the #[server] macro relocates the lint so #[expect]
// reports as unfulfilled. async is mandatory on a server function.
#[allow(clippy::unused_async, reason = "async is required by #[server]")]
#[server]
pub async fn add_user(username: String) -> Result<(), AppError> {
    use crate::store::ChallengeStore;

    let store = expect_context::<ChallengeStore>();
    store
        .add_user(&username, jiff::Timestamp::now())
        .map_err(with_status)?;
    leptos_axum::redirect(format!("/u/{username}").as_str());
    Ok(())
}

/// Get a user.
#[allow(clippy::unused_async, reason = "async is required by #[server]")]
#[server]
pub async fn get_user(username: String) -> Result<User, AppError> {
    use crate::store::ChallengeStore;

    let store = expect_context::<ChallengeStore>();
    store
        .get_user(&username)
        .ok_or_else(|| with_status(AppError::UserNotFound))
}

/// Confirm a found password; the first correct confirmation records the
/// solve and the leaderboard entry.
#[allow(clippy::unused_async, reason = "async is required by #[server]")]
#[server]
pub async fn confirm_password(username: String, password: String) -> Result<(), AppError> {
    use crate::store::{ChallengeStore, ConfirmOutcome};

    let store = expect_context::<ChallengeStore>();
    match store.confirm(&username, &password, jiff::Timestamp::now()) {
        ConfirmOutcome::NotFound => Err(with_status(AppError::UserNotFound)),
        ConfirmOutcome::Throttled => Err(with_status(AppError::ConfirmThrottled)),
        ConfirmOutcome::Incorrect => Err(with_status(AppError::WrongPassword)),
        ConfirmOutcome::Confirmed => Ok(()),
    }
}

/// Round a solve duration for display: days at the largest, whole seconds
/// at the smallest.
fn format_time_to_solve(span: Span) -> String {
    let rounded = span
        .round(
            SpanRound::new()
                .largest(Unit::Day)
                .smallest(Unit::Second)
                .days_are_24_hours(),
        )
        .expect("solve durations fit well within Span rounding limits");
    format!("{rounded:#}")
}

/// Read the current leaderboard.
#[allow(clippy::unused_async, reason = "async is required by #[server]")]
#[server]
pub async fn get_leaders() -> Result<Vec<Completion>, AppError> {
    use crate::store::ChallengeStore;

    Ok(expect_context::<ChallengeStore>().leaders())
}

/// Read the full roster of registered players.
#[allow(clippy::unused_async, reason = "async is required by #[server]")]
#[server]
pub async fn get_roster() -> Result<Vec<RosterEntry>, AppError> {
    use crate::store::ChallengeStore;

    Ok(expect_context::<ChallengeStore>().roster())
}

#[must_use]
pub fn shell(options: LeptosOptions) -> impl IntoView {
    view! {
        <!DOCTYPE html>
        <html lang="en">
            <head>
                <meta charset="utf-8" />
                <meta name="viewport" content="width=device-width, initial-scale=1" />
                <AutoReload options=options.clone() />
                <HydrationScripts options />
                <link rel="shortcut icon" type="image/ico" href="/favicon.ico" />
                <MetaTags />
            </head>
            <body>
                <App />
            </body>
        </html>
    }
}

#[component]
#[expect(
    clippy::module_name_repetitions,
    reason = "App is the conventional root component name"
)]
pub fn App() -> impl IntoView {
    // Provides context that manages stylesheets, titles, meta tags, etc.
    provide_meta_context();

    view! {
        <Stylesheet href="https://cdn.jsdelivr.net/npm/water.css@2/out/water.css" />
        <Stylesheet id="leptos" href="/pkg/challenge.css" />

        // sets the document title
        <Title text="Challenge: Find the Password" />

        // content for this welcome page
        <Router>
            <main>
                <Routes fallback=|| "Page not found.".into_view()>
                    <Route path=path!("") view=HomePage />
                    // Also matches with a trailing slash; the router tolerates it.
                    <Route path=path!("/u/:username") view=UserPage />
                </Routes>
            </main>
        </Router>
    }
}
/// Route parameters for [`UserPage`].
#[derive(Params, PartialEq, Clone)]
struct UserParams {
    username: Option<String>,
}

/// A user's specific page
#[component]
fn UserPage() -> impl IntoView {
    let params = use_params::<UserParams>();
    let confirm = ServerAction::<ConfirmPassword>::new();
    // Keyed on the action version so a confirmation submission refetches the
    // user, flipping the page to the solved view on success.
    let user = Resource::new(
        move || {
            (
                params.get().ok().and_then(|params| params.username),
                confirm.version().get(),
            )
        },
        |(username, _)| async {
            match username {
                None => None,
                Some(u) => Some(get_user(u).await),
            }
        },
    );
    let confirm_error = move || {
        confirm
            .value()
            .get()
            .and_then(Result::err)
            .map(|error| error.to_string())
    };

    view! {
        <Suspense fallback=move || {
            view! { <p>"Loading..."</p> }
        }>
            {move || {
                user.get()
                    .map(|user| {
                        if let Some(Ok(user)) = user {
                            Either::Left(
                                view! {
                                    <h1 id="username">"Hi, "{user.username.clone()}"!"</h1>
                                    <p>
                                        "Glad to have you join us for this challenge! Download your password file by clicking the link below."
                                    </p>
                                    <a
                                        class="button"
                                        href=format!("/u/{}/passwords.txt", &user.username)
                                        download="passwords.txt"
                                    >
                                        "Get your passwords.txt"
                                    </a>
                                    {if let Some(solved_at) = user.solved_at {
                                        Either::Left(
                                            view! {
                                                <h2>"You solved it! 🎉"</h2>
                                                <p>
                                                    "You confirmed the password after "
                                                    {format_time_to_solve(solved_at - user.created_at)}
                                                    ", using "<code>{user.hits_before_solved}</code>
                                                    " attempts."
                                                </p>
                                            },
                                        )
                                    } else {
                                        Either::Right(
                                            view! {
                                                <h2>"Found the password?"</h2>
                                                <p>
                                                    "Checking a password in the URL only tells you "
                                                    <code>"true"</code>" or "<code>"false"</code>
                                                    ". To actually solve the challenge, enter the password you found below. Guesses here count as attempts too."
                                                </p>
                                                {move || {
                                                    confirm_error()
                                                        .map(|msg| {
                                                            view! {
                                                                <div class="error">
                                                                    <p>{msg}</p>
                                                                </div>
                                                            }
                                                        })
                                                }}
                                                <ActionForm action=confirm>
                                                    <input
                                                        type="hidden"
                                                        name="username"
                                                        value=user.username.clone()
                                                    />
                                                    <input
                                                        type="text"
                                                        name="password"
                                                        placeholder="The password you found"
                                                        required
                                                    />
                                                    <input type="submit" value="Confirm password" />
                                                </ActionForm>
                                            },
                                        )
                                    }}
                                },
                            )
                        } else {
                            Either::Right(view! { <Redirect path="/" /> })
                        }
                    })
            }}
        </Suspense>
    }
}

/// Renders the home page of your application.
#[component]
fn HomePage() -> impl IntoView {
    let add_user = ServerAction::<AddUser>::new();
    let error_msg = move || {
        add_user
            .value()
            .get()
            .and_then(Result::err)
            .map(|error| error.to_string())
    };

    // Blocking so the leaderboard and roster are part of the server-rendered
    // HTML instead of popping in after hydration.
    let leaders = OnceResource::new_blocking(get_leaders());
    let roster = OnceResource::new_blocking(get_roster());

    view! {
        <h1 id="finding-the-password">"Finding the password"</h1>
        <p>
            "I have a text file with 60,000 passwords (one password per line). I seem to have lost my password in this file. Can you help me find it?"
        </p>
        <h2 id="how-to-play">"How to play"</h2>
        <ol>
            <li>"Create a new user by choosing a username below."</li>
            <li>
                "On your user page, click \"Get your passwords.txt\" to get your personal list of passwords."
            </li>
            <li>
                "Check if a password is the one I lost by checking this website with the target password in the URL following this template:"
                <pre>
                    <code>"https://challenge.hsiao.dev/u/{username}/check/{password}"</code>
                </pre>"where "<code>"username"</code>" is your username and "<code>"password"</code>
                " is the password you are checking." <br /> "For example, if my username was "
                <code>"john"</code> " and I wanted to check password " <code>"asdf"</code>
                ", the URL would be " <code>"https://challenge.hsiao.dev/u/john/check/asdf"</code>
                "."
            </li>
            <li>
                "If the password is correct, you'll see "<code>"true"</code>
                ", and if it is incorrect, you will see "<code>"false"</code>"."
            </li>
            <li>
                "Seeing "<code>"true"</code>
                " means you found the password, but you're not done! Return to your user page, which lives at:"
                <pre>
                    <code>"https://challenge.hsiao.dev/u/{username}"</code>
                </pre>"For example, if your username was "<code>"john"</code>
                ", your user page would be " <code>"https://challenge.hsiao.dev/u/john"</code>
                ". Enter the password you found in the confirmation box there. The challenge only counts as solved once you confirm the password."
            </li>
        </ol>
        <h2 id="rules">"Rules"</h2>
        <ul>
            <li>
                "No using AI to solve the problem. The purpose of this is education! Feel free to consult the web and AI to learn, but do not let an AI rob you of the learning experience."
            </li>
            <li>
                "No sharing a solution with each other, everyone has to do their own work, but you're free to collaborate."
            </li>
            <li>
                "If you can solve it, you have to share with me what you did, along with some reflection about the challenge (e.g., what you learned, what was hard, what was surprising)."
            </li>
            <li>"Only use the url with your own name in it, don't impersonate others."</li>
            <li>
                "There is no limit to how many times you can try. If you want to completely restart, make a new user."
            </li>
        </ul>
        <h2 id="extra-challenge">"Want an extra challenge?"</h2>
        <p>
            "Finding the password once is satisfying. Finding it "<em>"fast"</em>
            " is a different game entirely. The leaderboard below tracks how long every solve takes, from joining the challenge until the password is confirmed on your user page. Every guess counts as an attempt, whether it goes through the check URL or the confirmation box, so a sloppy script that keeps guessing after it finds the password keeps inflating your attempt count. So here's the real question: could you build a solution that finds the password for a brand-new user in under five minutes? What about under two minutes? Climb to the top of the board and find out."
        </p>
        <h2 id="lets-go">"Let's go!"</h2>

        {move || {
            error_msg()
                .map(|msg| {
                    view! {
                        <div class="error">
                            <p>{msg}</p>
                        </div>
                    }
                })
        }}
        <ActionForm action=add_user>
            <input type="text" placeholder="Your username" name="username" required />
            <input type="submit" value="Join challenge" />
        </ActionForm>

        <h2>Leaderboard</h2>
        <table>
            <thead>
                <tr>
                    <th>"Username"</th>
                    <th>"Time to Solve"</th>
                    <th style="text-align: right;">"Attempts to Solve"</th>
                </tr>
            </thead>
            <tbody>
                <Suspense fallback=move || {}>
                    {move || {
                        leaders
                            .get()
                            .map(|l| {
                                l.map(|v| {
                                    v.into_iter()
                                        .map(|completion| {
                                            view! {
                                                <tr>
                                                    <td>{completion.username}</td>
                                                    <td>{format_time_to_solve(completion.time_to_solve)}</td>
                                                    <td style="text-align: right;">
                                                        <code>{completion.attempts_to_solve}</code>
                                                    </td>
                                                </tr>
                                            }
                                        })
                                        .collect_view()
                                })
                            })
                    }}
                </Suspense>
            </tbody>
        </table>

        <h2>"All players"</h2>
        <table>
            <thead>
                <tr>
                    <th>"Username"</th>
                    <th>"Solved"</th>
                    <th style="text-align: right;">"Attempts"</th>
                </tr>
            </thead>
            <tbody>
                <Suspense fallback=move || {}>
                    {move || {
                        roster
                            .get()
                            .map(|r| {
                                r.map(|v| {
                                    v.into_iter()
                                        .map(|entry| {
                                            view! {
                                                <tr>
                                                    <td>{entry.username}</td>
                                                    <td>{if entry.solved { "yes" } else { "no" }}</td>
                                                    <td style="text-align: right;">
                                                        <code>{entry.attempts}</code>
                                                    </td>
                                                </tr>
                                            }
                                        })
                                        .collect_view()
                                })
                            })
                    }}
                </Suspense>
            </tbody>
        </table>
    }
}

// Integration tests for the confirmation server function's HTTP contract.
// They drive the real server-fn route the same way main() mounts it, so
// they lock the semantic status codes (and the plain-form 302 fallback)
// that the ResponseOptions override in with_status produces. In-crate so
// they can read the crate-private secret as the correct-password oracle.
#[cfg(all(test, feature = "ssr"))]
mod tests {
    use axum::{
        Router,
        body::Body,
        http::{HeaderMap, Request, StatusCode, header},
        routing::post,
    };
    use jiff::Timestamp;
    use leptos::{prelude::provide_context, server_fn::ServerFn};
    use tower::ServiceExt;

    use super::ConfirmPassword;
    use crate::store::ChallengeStore;

    /// Mount the server-fn route exactly as `main()` does, with the store
    /// provided through context.
    fn app(store: ChallengeStore) -> Router {
        Router::new().route(
            "/api/{*fn_name}",
            post(move |req: Request<Body>| {
                let store = store.clone();
                async move {
                    leptos_axum::handle_server_fns_with_context(
                        move || provide_context(store.clone()),
                        req,
                    )
                    .await
                }
            }),
        )
    }

    /// POST a confirmation the way a client would: form-encoded body, with
    /// the Accept header distinguishing the wasm client from a plain form.
    async fn confirm_over_http(
        router: Router,
        body: &str,
        accept: &str,
        referer: Option<&str>,
    ) -> (StatusCode, HeaderMap, String) {
        let mut request = Request::builder()
            .method("POST")
            .uri(ConfirmPassword::PATH)
            .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
            .header(header::ACCEPT, accept);
        if let Some(referer) = referer {
            request = request.header(header::REFERER, referer);
        }
        let response = router
            .oneshot(request.body(Body::from(body.to_owned())).unwrap())
            .await
            .unwrap();
        let status = response.status();
        let headers = response.headers().clone();
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        (status, headers, String::from_utf8(bytes.to_vec()).unwrap())
    }

    #[tokio::test]
    async fn confirm_maps_client_mistakes_to_4xx() {
        let store = ChallengeStore::new();
        store.add_user("alice", Timestamp::now()).unwrap();
        let router = app(store);

        let (status, _, body) = confirm_over_http(
            router.clone(),
            "username=ghost&password=x",
            "application/json",
            None,
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body, "\"UserNotFound\"");

        let (status, _, body) = confirm_over_http(
            router.clone(),
            "username=alice&password=wrong",
            "application/json",
            None,
        )
        .await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(body, "\"WrongPassword\"");

        // Inside the cooldown armed by the evaluated guess above.
        let (status, headers, body) = confirm_over_http(
            router,
            "username=alice&password=wrong",
            "application/json",
            None,
        )
        .await;
        assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(headers.get(header::RETRY_AFTER).unwrap(), "10");
        assert_eq!(body, "\"ConfirmThrottled\"");
    }

    #[tokio::test]
    async fn confirm_answers_the_correct_password_with_200() {
        let store = ChallengeStore::new();
        store.add_user("bob", Timestamp::now()).unwrap();
        let secret = store.get_user("bob").unwrap().secret;
        let router = app(store.clone());

        let (status, _, _) = confirm_over_http(
            router,
            &format!("username=bob&password={secret}"),
            "application/json",
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert!(store.get_user("bob").unwrap().solved_at.is_some());
        assert_eq!(store.leaders().len(), 1);
    }

    // A plain form post (JS disabled) is answered with a 302 back to the
    // referring page; the status override must stand down for it or the
    // no-JS flow would render a raw error body instead of the page.
    #[tokio::test]
    async fn confirm_redirects_plain_form_posts_even_on_errors() {
        let store = ChallengeStore::new();
        store.add_user("carol", Timestamp::now()).unwrap();
        let router = app(store);

        let (status, headers, _) = confirm_over_http(
            router,
            "username=carol&password=wrong",
            "text/html",
            Some("http://localhost:3000/u/carol"),
        )
        .await;
        assert_eq!(status, StatusCode::FOUND);
        let location = headers.get(header::LOCATION).unwrap().to_str().unwrap();
        assert!(location.starts_with("http://localhost:3000/u/carol"));
    }
}
