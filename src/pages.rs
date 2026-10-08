//! The HTML pages players click through: joining on the home page, then
//! downloading their password file and confirming the password on their
//! own page.
//!
//! Forms post back to the URL of the page they live on. Success redirects
//! with 303 See Other; a mistake re-renders the same page with the error
//! message and a 4xx status.

use jiff::{Span, SpanRound, Unit};
use serde::Deserialize;
use topcoat::{
    Result,
    context::Cx,
    router::{
        HeaderValue, Slot,
        content::Form,
        error::{RouterErrorExt, not_found, see_other},
        header::RETRY_AFTER,
        href, layout, page, path_param,
    },
    view::{Unescaped, View, component, view},
};

use crate::{
    Username, clock,
    error::AppError,
    store,
    store::ConfirmOutcome,
    user::{CONFIRM_COOLDOWN, User},
};

/// Water.css palette overrides, inlined because they are a few kilobytes
/// and inlining saves the request a separate stylesheet would cost.
const STYLE: &str = include_str!("../style/main.css");

#[layout("/")]
pub async fn layout(slot: Slot<'_>) -> Result<impl View> {
    Ok(view! {
        <!DOCTYPE html>
        <html lang="en">
            <head>
                <meta charset="utf-8">
                <meta name="viewport" content="width=device-width, initial-scale=1">
                <title>"Challenge: Find the Password"</title>
                <link
                    rel="stylesheet"
                    href="https://cdn.jsdelivr.net/npm/water.css@2/out/water.css"
                >
                // A compile-time constant from this crate, so it is trusted.
                <style>(Unescaped::new_unchecked(STYLE))</style>
                topcoat::dev::script()
            </head>
            <body><main>(slot)</main></body>
        </html>
    })
}

#[derive(Debug, Deserialize)]
pub struct JoinForm {
    username: String,
}

#[derive(Debug, Deserialize)]
pub struct ConfirmForm {
    password: String,
}

#[page("/")]
pub async fn home() -> Result<impl View> {
    Ok(view! { home_body() })
}

/// Register a player, then send them to their page.
#[page(POST "/")]
pub async fn join(cx: &Cx, Form(form): Form<JoinForm>) -> Result<impl View> {
    match store(cx).add_user(&form.username, clock(cx).now()) {
        Ok(()) => Err(see_other(href!(player, Username(&form.username)).resolve(cx)).into()),
        Err(error) => Ok(view! {
            (error.status())
            home_body(error: Some(error), username: form.username)
        }),
    }
}

/// A player's own page. Unknown players are sent home to join.
#[page("/u/{username}")]
pub async fn player(cx: &Cx) -> Result<impl View> {
    let user = store(cx)
        .get_user(path_param::<Username>(cx))
        .ok_or_else(|| see_other(href!(home).resolve(cx)))?;
    Ok(view! { player_body(user: user) })
}

/// Confirm a found password; the first correct confirmation records the
/// solve and the leaderboard entry.
#[page(POST "/u/{username}")]
pub async fn confirm(cx: &Cx, Form(form): Form<ConfirmForm>) -> Result<impl View> {
    let username = path_param::<Username>(cx);
    let error = match store(cx).confirm(username, &form.password, clock(cx).now()) {
        ConfirmOutcome::NotFound => return Err(not_found().into()),
        ConfirmOutcome::Confirmed => {
            return Err(see_other(href!(player, Username(username)).resolve(cx)).into());
        }
        ConfirmOutcome::Incorrect => AppError::WrongPassword,
        ConfirmOutcome::Throttled => AppError::ConfirmThrottled,
    };
    let user = store(cx).get_user(username).ok_or_not_found()?;
    // The upper bound on the wait; the exact remainder is not worth
    // threading through the domain types.
    let retry_after = (error == AppError::ConfirmThrottled)
        .then(|| (RETRY_AFTER, HeaderValue::from(CONFIRM_COOLDOWN.as_secs())));
    Ok(view! {
        (error.status())
        (retry_after)
        player_body(user: user, error: Some(error))
    })
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

#[component]
async fn error_message(error: Option<AppError>) -> Result<impl View> {
    Ok(view! {
        if let Some(error) = error {
            <div class="error"><p>(error.to_string())</p></div>
        }
    })
}

/// The home page: how to play, the join form, and the standings.
///
/// `error` and `username` re-render a rejected join with its message and
/// the name the player typed.
#[component]
async fn home_body(
    #[default] error: Option<AppError>,
    #[default] username: String,
) -> Result<impl View> {
    Ok(view! {
        how_to_play()
        error_message(error: error)
        <form method="post" action=(href!(join))>
            <input
                type="text"
                placeholder="Your username"
                name="username"
                value=(username)
                required=""
            >
            <input type="submit" value="Join challenge">
        </form>
        standings()
    })
}

/// The challenge and its rules, ending at the join form's heading.
#[component]
async fn how_to_play() -> Result<impl View> {
    Ok(view! {
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
                    <code>
                        "https://challenge.hsiao.dev/u/{username}/check/{password}"
                    </code>
                </pre>
                "where "
                <code>"username"</code>
                " is your username and "
                <code>"password"</code>
                " is the password you are checking."
                <br>
                "For example, if my username was "
                <code>"john"</code>
                " and I wanted to check password "
                <code>"asdf"</code>
                ", the URL would be "
                <code>"https://challenge.hsiao.dev/u/john/check/asdf"</code>
                "."
            </li>
            <li>
                "If the password is correct, you'll see "
                <code>"true"</code>
                ", and if it is incorrect, you will see "
                <code>"false"</code>
                "."
            </li>
            <li>
                "Seeing "
                <code>"true"</code>
                " means you found the password, but you're not done! Return to your user page, which lives at:"
                <pre><code>"https://challenge.hsiao.dev/u/{username}"</code></pre>
                "For example, if your username was "
                <code>"john"</code>
                ", your user page would be "
                <code>"https://challenge.hsiao.dev/u/john"</code>
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
            <li>
                "Only use the url with your own name in it, don't impersonate others."
            </li>
            <li>
                "There is no limit to how many times you can try. If you want to completely restart, make a new user."
            </li>
        </ul>
        <h2 id="extra-challenge">"Want an extra challenge?"</h2>
        <p>
            "Finding the password once is satisfying. Finding it "
            <em>"fast"</em>
            " is a different game entirely. The leaderboard below tracks how long every solve takes, from joining the challenge until the password is confirmed on your user page. So here's a tougher challenge: could you build a solution that finds the password for a brand-new user in under five minutes? What about under one minute? 20 seconds? Climb to the top of the board and find out."
        </p>
        <h2 id="lets-go">"Let's go!"</h2>
    })
}

/// The leaderboard of solves and the roster of every player.
#[component]
async fn standings(cx: &Cx) -> Result<impl View> {
    let leaders = store(cx).leaders();
    let roster = store(cx).roster();
    Ok(view! {
        <h2>"Leaderboard"</h2>
        <table>
            <thead>
                <tr>
                    <th>"Username"</th>
                    <th>"Time to Solve"</th>
                    <th style="text-align: right;">"Attempts to Solve"</th>
                </tr>
            </thead>
            <tbody>
                for completion in leaders {
                    <tr>
                        <td>(completion.username)</td>
                        <td>(format_time_to_solve(completion.time_to_solve))</td>
                        <td style="text-align: right;">
                            <code>(completion.attempts_to_solve)</code>
                        </td>
                    </tr>
                }
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
                for entry in roster {
                    <tr>
                        <td>(entry.username)</td>
                        <td>(if entry.solved { "yes" } else { "no" })</td>
                        <td style="text-align: right;">
                            <code>(entry.attempts)</code>
                        </td>
                    </tr>
                }
            </tbody>
        </table>
    })
}

/// A player's page: the password file download, then either the
/// confirmation form or, once solved, how the solve went.
#[component]
async fn player_body(user: User, #[default] error: Option<AppError>) -> Result<impl View> {
    Ok(view! {
        <h1 id="username">
            "Hi, "
            (&user.username)
            "!"
        </h1>
        <p>
            "Glad to have you join us for this challenge! Download your password file by clicking the link below."
        </p>
        <a
            class="button"
            href=(href!(crate::http::passwords_txt, Username(&user.username)))
            download="passwords.txt"
        >
            "Get your passwords.txt"
        </a>
        match user.solved_at {
            Some(solved_at) => {
                <h2>"You solved it! 🎉"</h2>
                <p>
                    "You confirmed the password after "
                    (format_time_to_solve(solved_at - user.created_at))
                    ", using "
                    <code>(user.hits_before_solved)</code>
                    " attempts."
                </p>
            }
            None => {
                <h2>"Found the password?"</h2>
                <p>
                    "Checking a password in the URL only tells you "
                    <code>"true"</code>
                    " or "
                    <code>"false"</code>
                    ". To actually solve the challenge, enter the password you found below. Guesses here count as attempts too."
                </p>
                error_message(error: error)
                <form method="post" action=(href!(confirm, Username(&user.username)))>
                    <input
                        type="text"
                        name="password"
                        placeholder="The password you found"
                        required=""
                    >
                    <input type="submit" value="Confirm password">
                </form>
            }
        }
    })
}

// The form flows over HTTP: redirects on success, and the semantic 4xx
// statuses a rejected form re-renders with.
#[cfg(test)]
mod tests {
    use jiff::SignedDuration;
    use topcoat::router::{StatusCode, header};

    use crate::testing::TestApp;

    #[tokio::test]
    async fn joining_redirects_to_the_new_players_page() {
        let app = TestApp::new();
        let reply = app.post_form("/", "username=alice").await;
        assert_eq!(reply.status, StatusCode::SEE_OTHER);
        assert_eq!(reply.header(&header::LOCATION), "/u/alice");
        assert!(app.store.get_user("alice").is_some());

        let page = app.get("/u/alice").await;
        assert_eq!(page.status, StatusCode::OK);
        assert!(page.body.contains("Hi, alice!"));
        assert!(page.body.contains(r#"href="/u/alice/passwords.txt""#));
    }

    #[tokio::test]
    async fn joining_with_an_invalid_name_is_422_and_keeps_the_input() {
        let app = TestApp::new();
        let reply = app.post_form("/", "username=no%2Fslash").await;
        assert_eq!(reply.status, StatusCode::UNPROCESSABLE_ENTITY);
        assert!(reply.body.contains("Username must be 3-32 characters"));
        assert!(reply.body.contains(r#"value="no/slash""#));
        assert_eq!(app.store.roster(), vec![]);
    }

    #[tokio::test]
    async fn joining_with_a_taken_name_is_409() {
        let app = TestApp::new();
        app.store.add_user("alice", app.clock.now()).unwrap();
        let reply = app.post_form("/", "username=alice").await;
        assert_eq!(reply.status, StatusCode::CONFLICT);
        assert!(reply.body.contains("That username is already taken."));
    }

    #[tokio::test]
    async fn an_unknown_players_page_redirects_home() {
        let reply = TestApp::new().get("/u/ghost").await;
        assert_eq!(reply.status, StatusCode::SEE_OTHER);
        assert_eq!(reply.header(&header::LOCATION), "/");
    }

    #[tokio::test]
    async fn confirming_for_an_unknown_player_is_404() {
        let reply = TestApp::new().post_form("/u/ghost", "password=x").await;
        assert_eq!(reply.status, StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn confirming_maps_mistakes_to_4xx() {
        let app = TestApp::new();
        app.store.add_user("alice", app.clock.now()).unwrap();

        let reply = app.post_form("/u/alice", "password=wrong").await;
        assert_eq!(reply.status, StatusCode::UNPROCESSABLE_ENTITY);
        assert!(
            reply
                .body
                .contains("That's not the password. Keep hunting!")
        );

        // Inside the cooldown armed by the evaluated guess above.
        let reply = app.post_form("/u/alice", "password=wrong").await;
        assert_eq!(reply.status, StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(reply.header(&header::RETRY_AFTER), "10");
        assert!(reply.body.contains("Whoa, slow down!"));
        assert_eq!(app.store.get_user("alice").unwrap().hits_before_solved, 1);
    }

    #[tokio::test]
    async fn confirming_the_password_solves_and_redirects() {
        let app = TestApp::new();
        app.store.add_user("bob", app.clock.now()).unwrap();
        let secret = app.store.get_user("bob").unwrap().secret;

        let reply = app.post_form("/u/bob", &format!("password={secret}")).await;
        assert_eq!(reply.status, StatusCode::SEE_OTHER);
        assert_eq!(reply.header(&header::LOCATION), "/u/bob");
        assert_eq!(app.store.leaders().len(), 1);

        let page = app.get("/u/bob").await;
        assert!(page.body.contains("You solved it!"));
        assert!(!page.body.contains("Confirm password"));
    }

    // Retry-After is a promise: waiting exactly that long gets the next
    // confirmation evaluated.
    #[tokio::test]
    async fn a_throttled_confirmation_is_evaluated_after_retry_after() {
        let app = TestApp::new();
        app.store.add_user("alice", app.clock.now()).unwrap();
        let secret = app.store.get_user("alice").unwrap().secret;
        app.post_form("/u/alice", "password=wrong").await;

        let throttled = app
            .post_form("/u/alice", &format!("password={secret}"))
            .await;
        assert_eq!(throttled.status, StatusCode::TOO_MANY_REQUESTS);
        let wait: i64 = throttled.header(&header::RETRY_AFTER).parse().unwrap();

        app.clock.advance(SignedDuration::from_secs(wait));
        let reply = app
            .post_form("/u/alice", &format!("password={secret}"))
            .await;
        assert_eq!(reply.status, StatusCode::SEE_OTHER);
    }

    #[tokio::test]
    async fn the_solve_time_runs_from_joining_to_confirming() {
        let app = TestApp::new();
        app.post_form("/", "username=eve").await;
        let secret = app.store.get_user("eve").unwrap().secret;

        app.clock.advance(SignedDuration::from_secs(90));
        app.post_form("/u/eve", &format!("password={secret}")).await;

        let page = app.get("/u/eve").await.body;
        assert!(
            page.contains("You confirmed the password after 1m 30s"),
            "{page}"
        );
        let home = app.get("/").await.body;
        assert!(home.contains("<td>eve</td><td>1m 30s</td>"), "{home}");
    }

    #[tokio::test]
    async fn the_home_page_lists_every_player_and_the_leaderboard() {
        let app = TestApp::new();
        app.store.add_user("solver", app.clock.now()).unwrap();
        app.store.add_user("grinder", app.clock.now()).unwrap();
        let secret = app.store.get_user("solver").unwrap().secret;
        app.store.confirm("solver", &secret, app.clock.now());
        for _ in 0..3 {
            app.store.check("grinder", "wrong");
        }

        let body = app.get("/").await.body;
        let tables: Vec<&str> = body.split("<tbody>").skip(1).collect();
        assert_eq!(tables.len(), 2, "leaderboard and roster");
        assert!(tables[0].contains("<td>solver</td>"));
        assert!(!tables[0].contains("grinder"));
        assert!(tables[1].contains(
            "<td>grinder</td><td>no</td><td style=\"text-align: right;\"><code>3</code>"
        ));
        assert!(tables[1].contains("<td>solver</td><td>yes</td>"));
    }
}
