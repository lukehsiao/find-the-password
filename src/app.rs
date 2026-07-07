use jiff::{SpanRound, Unit};
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

/// Add a new user to the user map.
// allow, not expect: the #[server] macro relocates the lint so #[expect]
// reports as unfulfilled. async is mandatory on a server function.
#[allow(clippy::unused_async, reason = "async is required by #[server]")]
#[server]
pub async fn add_user(username: String) -> Result<(), AppError> {
    use crate::store::ChallengeStore;

    let store = expect_context::<ChallengeStore>();
    store.add_user(&username, jiff::Timestamp::now())?;
    leptos_axum::redirect(format!("/u/{username}").as_str());
    Ok(())
}

/// Get a user.
#[allow(clippy::unused_async, reason = "async is required by #[server]")]
#[server]
pub async fn get_user(username: String) -> Result<User, AppError> {
    use crate::store::ChallengeStore;

    let store = expect_context::<ChallengeStore>();
    store.get_user(&username).ok_or(AppError::UserNotFound)
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
    let user = Resource::new(
        move || params.get().ok().and_then(|params| params.username),
        |username| async {
            match username {
                None => None,
                Some(u) => Some(get_user(u).await),
            }
        },
    );

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
            " is a different game entirely. The leaderboard below tracks how long every solve takes from downloading the password file until the solve. So here's the real question: could you build a solution that finds the password for a brand-new user in under five minutes? What about under two minutes? Climb to the top of the board and find out."
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
                                                    <td>
                                                        {
                                                            let rounded = completion
                                                                .time_to_solve
                                                                .round(
                                                                    SpanRound::new()
                                                                        .largest(Unit::Day)
                                                                        .smallest(Unit::Second)
                                                                        .days_are_24_hours(),
                                                                )
                                                                .unwrap();
                                                            format!("{rounded:#}")
                                                        }
                                                    </td>
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
