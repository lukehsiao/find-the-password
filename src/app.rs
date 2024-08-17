use leptos::*;
use leptos_meta::*;
use leptos_router::*;

use crate::{
    error_template::{AppError, ErrorTemplate},
    user::Completion,
};

/// Add a new user to the user map.
#[server(AddUser)]
pub async fn add_user(username: String) -> Result<(), ServerFnError> {
    if username.is_empty() {
        return Err(ServerFnError::ServerError(
            "username must not just be whitespace".to_string(),
        ));
    }
    use crate::state::AppState;
    use crate::user::User;
    let state = expect_context::<AppState>();
    dbg!(&state);
    if state.usermap.contains_key(&username) {
        Err(ServerFnError::ServerError(
            "user already exists".to_string(),
        ))
    } else {
        state.usermap.insert(username.clone(), User::new(username));
        // TODO: redirect to user page
        leptos_axum::redirect("/");
        Ok(())
    }
}

/// Read the current leaderboard.
#[server(GetLeaders)]
pub async fn get_leaders() -> Result<Vec<Completion>, ServerFnError> {
    use crate::state::AppState;
    let state = expect_context::<AppState>();
    Ok((*state.leaderboard).clone())
}

#[component]
pub fn App() -> impl IntoView {
    // Provides context that manages stylesheets, titles, meta tags, etc.
    provide_meta_context();

    view! {
        <Stylesheet href="https://cdn.jsdelivr.net/npm/water.css@2/out/water.css" />
        <Stylesheet id="leptos" href="/pkg/challenge.css" />

        // sets the document title
        <Title text="Challenge: Find the Password" />

        // content for this welcome page
        <Router fallback=|| {
            let mut outside_errors = Errors::default();
            outside_errors.insert_with_default_key(AppError::NotFound);
            view! { <ErrorTemplate outside_errors /> }.into_view()
        }>
            <main>
                <Routes>
                    <Route path="" view=HomePage />
                </Routes>
            </main>
        </Router>
    }
}

/// Renders the home page of your application.
#[component]
fn HomePage() -> impl IntoView {
    let add_user = Action::<AddUser, _>::server();
    let value = Signal::derive(move || add_user.value().get().unwrap_or_else(|| Ok(())));

    let leaders = create_resource(|| (), |_| async move { get_leaders().await });

    view! {
        <h1 id="finding-the-password">"Finding the password"</h1>
        <p>
            "I have a text file with 50,000 passwords (one password per line). I seem to have lost my password in this file. Can you help me find it?"
        </p>
        <h2 id="how-to-play">"How to Play"</h2>
        <ol>
            <li>"Create a new user by choosing a username and entering a key from Luke."</li>
            <li>
                "On your user page, click \"Download passwords\" to get your personal list of passwords."
            </li>
            <li>
                "Check if a password is the one I lost by checking this website with it in the URL following this template:"
                <pre>
                    <code>"https://challenge.hsiao.dev/u/{username}/check/{password}"</code>
                </pre>"where "<code>"username"</code>" is your username and "<code>"password"</code>
                is the password you are checking.
            </li>
            <li>
                "If the password is correct, you'll see "<code>"True"</code>
                ", and if it is incorrect, you will see "<code>"False"</code>"."
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
                "There is no limit to how many times you can try. If you want to completely restart, we can reset your user."
            </li>
        </ul>
        <h2 id="lets-go">"Let's Go!"</h2>

        <ErrorBoundary fallback=move |error| {
            view! {
                <div class="error">
                    <p>
                        {move || {
                            format!(
                                "{}",
                                error
                                    .get()
                                    .into_iter()
                                    .next()
                                    .unwrap()
                                    .1
                                    .to_string()
                                    .strip_prefix("error running server function: ")
                                    .unwrap(),
                            )
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
                <Transition fallback=move || {
                    view! {}
                }>
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
                                                    <td>{completion.time_to_solve.to_string()}</td>
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
