use std::sync::{Arc, LazyLock};

use jiff::{SpanRound, Unit};
use leptos::{either::Either, prelude::*};
use leptos_meta::{MetaTags, Stylesheet, Title, provide_meta_context};
use leptos_router::{
    NavigateOptions,
    components::{Redirect, Route, Router, Routes},
    hooks::{use_navigate, use_params_map},
    path,
};

use crate::user::{Completion, User, Users};

/// Enforce that a username matches this specific pattern.
#[cfg(feature = "ssr")]
fn valid_username(username: &str) -> bool {
    use regex::Regex;

    static RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[a-zA-Z0-9]{3,32}$").unwrap());
    RE.is_match(username)
}

/// Add a new user to the user map.
#[allow(clippy::unused_async)]
#[server(AddUser, "/api")]
pub async fn add_user(username: String) -> Result<(), ServerFnError> {
    use crate::user::{User, Users};
    use std::sync::Arc;
    use tracing::info;

    // Guard classes for username validation
    if !valid_username(&username) {
        return Err(ServerFnError::ServerError(
            "Error: username must be simple ASCII characters (a-zA-Z) or numbers (0-9) with no whitespace, and between 3 and 32 characters in length.".to_string(),
        ));
    }

    let usermap = expect_context::<Arc<Users>>();
    if usermap.contains_key(&username) {
        Err(ServerFnError::ServerError(
            "username is already taken".to_string(),
        ))
    } else {
        info!(?username, "added user");
        leptos_axum::redirect(format!("/u/{}", &username).as_str());
        usermap.insert(username.clone(), User::new(username));
        Ok(())
    }
}

/// Get a user.
#[allow(clippy::unused_async)]
#[server(GetUser, "/api")]
pub async fn get_user(username: String) -> Result<User, ServerFnError> {
    if username.is_empty() {
        return Err(ServerFnError::ServerError(
            "username must not just be whitespace".to_string(),
        ));
    }
    let usermap = expect_context::<Arc<Users>>();
    if let Some(user) = usermap.get(&username) {
        Ok((*user).clone())
    } else {
        Err(ServerFnError::ServerError(
            "no user with that username".to_string(),
        ))
    }
}

/// Get a user's password file
#[allow(clippy::unused_async)]
#[server(GetUserPasswords, "/api")]
pub async fn get_user_passwords(username: String) -> Result<String, ServerFnError> {
    if username.is_empty() {
        return Err(ServerFnError::ServerError(
            "username must not just be whitespace".to_string(),
        ));
    }
    let usermap = expect_context::<Arc<Users>>();
    if let Some(user) = usermap.get(&username) {
        Ok(user.get_passwords())
    } else {
        // No user, just go home
        Err(ServerFnError::ServerError(
            "no user with that username".to_string(),
        ))
    }
}

/// Read the current leaderboard.
#[allow(clippy::unused_async)]
#[server(GetLeaders, "/api")]
pub async fn get_leaders() -> Result<Vec<Completion>, ServerFnError> {
    use crate::user::Completion;
    use std::sync::{Arc, Mutex};
    let leaders = expect_context::<Arc<Mutex<Vec<Completion>>>>()
        .lock()
        .unwrap()
        .clone();
    Ok(leaders)
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
#[allow(clippy::module_name_repetitions)]
#[must_use]
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
                    <Route path=path!("/u/:username") view=UserPage />
                    <Route path=path!("/u/:username/") view=RedirUserPage />
                </Routes>
            </main>
        </Router>
    }
}
/// Redirect to a user's specific page
#[component(transparent)]
fn RedirUserPage() -> impl IntoView {
    let params = use_params_map();
    let username = params.get().get("username");
    let navigate = use_navigate();

    match username {
        Some(u) => navigate(
            &format!("/u/{u}"),
            NavigateOptions {
                replace: true,
                ..Default::default()
            },
        ),
        None => navigate("/", NavigateOptions::default()),
    }
}

/// A user's specific page
#[component]
fn UserPage() -> impl IntoView {
    let params = use_params_map();
    let user = Resource::new(
        move || params.get().get("username"),
        |username| async {
            match username {
                None => None,
                Some(u) => Some(get_user(u).await),
            }
        },
    );

    view! {
        <Transition fallback=move || {
            view! { <p>"Error?"</p> }
        }>
            {move || {
                user.get()
                    .map(|user| {
                        if let Some(Ok(user)) = user {
                            let username = user.username.clone();
                            Either::Left(
                                view! {
                                    <h1 id="username">"Hi, "{username.clone()}"!"</h1>
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
        </Transition>
    }
}

/// Renders the home page of your application.
#[allow(clippy::too_many_lines)]
#[component]
fn HomePage() -> impl IntoView {
    let add_user = ServerAction::<AddUser>::new();
    let value = Signal::derive(move || add_user.value().get().unwrap_or(Ok(())));

    let leaders = Resource::new(|| (), |()| async move { get_leaders().await });

    view! {
        <h1 id="finding-the-password">"Finding the password"</h1>
        <p>
            "I have a text file with 60,000 passwords (one password per line). I seem to have lost my password in this file. Can you help me find it?"
        </p>
        <h2 id="how-to-play">"How to Play"</h2>
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
                is the password you are checking.
            </li>
            <li>
                "If the password is correct, you'll see "<code>"true"</code>
                ", and if it is incorrect, you will see "<code>"false"</code>"."
            </li>
        </ol>
        <h2 id="rules">"Rules"</h2>
        <ul>
            <li>
                "No sharing a solution with each other, everyone has to do their own work, but you're free to collaborate."
            </li>
            <li>"If you can solve it, you have to share with me what you did."</li>
            <li>"Only use the url with your own name in it, don't impersonate others."</li>
            <li>
                "There is no limit to how many times you can try. If you want to completely restart, make a new user."
            </li>
        </ul>
        <h2 id="lets-go">"Let's Go!"</h2>

        <ErrorBoundary fallback=move |error| {
            view! {
                <div class="error">
                    <p>
                        {move || {
                            let err = error.get().into_iter().next().unwrap().1.to_string();
                            err.strip_prefix("ServerError|Error: ").unwrap().to_string()
                        }}
                    </p>
                </div>
                <ActionForm action=add_user>
                    <input type="text" placeholder="Your username" name="username" required />
                    <input type="submit" value="Join challenge" />
                </ActionForm>
            }
        }>
            <div>{value}</div>
            <ActionForm action=add_user>
                <input type="text" placeholder="Your username" name="username" required />
                <input type="submit" value="Join challenge" />
            </ActionForm>
        </ErrorBoundary>

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
                <Transition fallback=move || {}>
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
                                                                    SpanRound::new().largest(Unit::Hour).smallest(Unit::Second),
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
                </Transition>
            </tbody>
        </table>
    }
}
