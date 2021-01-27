#![feature(proc_macro_hygiene, decl_macro)]

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

use chrono::Utc;
use rocket::{get, routes, State};
use rocket_contrib::serve::StaticFiles;

#[derive(Debug)]
struct HitCount {
    total_hits: AtomicUsize,
    hits: Mutex<HashMap<String, usize>>,
    eligible: Mutex<HashSet<&'static str>>,
    solved: Mutex<HashSet<String>>,
    success_count: AtomicUsize,
}

const PASS: &str = "ee21c52cba80a3b9bb1e237c3c84166f";

#[get("/total")]
fn total(count: State<HitCount>) -> String {
    format!(
        "Total hits: {}",
        count.total_hits.load(Ordering::Relaxed).to_string()
    )
}

#[get("/status/<name>")]
fn status(name: String, count: State<HitCount>) -> String {
    if let Some(attempts) = count.hits.lock().unwrap().get(&name) {
        format!("{} has made {} attempts.", name, attempts)
    } else {
        format!("Invalid name: {}", name)
    }
}

#[get("/01/<name>/<pass>")]
fn check(name: String, pass: String, count: State<HitCount>) -> String {
    count.total_hits.fetch_add(1, Ordering::Relaxed);

    // Stop tracking attempts after it has been solved.
    if !count.solved.lock().unwrap().contains(&name) {
        *count.hits.lock().unwrap().entry(name.clone()).or_insert(0) += 1;
    }

    let eligible = count.eligible.lock().unwrap();
    if eligible.contains(name.as_str()) {
        if pass.as_str() == PASS {
            // Log a sucessful solve to stderr
            if !count.solved.lock().unwrap().contains(&name) {
                count.success_count.fetch_add(1, Ordering::Relaxed);
                count.solved.lock().unwrap().insert(name.to_string());

                let success_count = count.success_count.load(Ordering::Relaxed);
                let attempts = *count.hits.lock().unwrap().get(&name).unwrap();

                eprintln!(
                    "[SUCCESS] {} got {} place with {} attempts at {}",
                    name,
                    success_count,
                    attempts,
                    Utc::now().to_rfc3339()
                );
            }

            "Yes\n".to_string()
        } else {
            "No\n".to_string()
        }
    } else {
        format!(
            "{}, you're not eligible to try this challenge.\nAre you using the right name?\n",
            name
        )
    }
}

#[get("/")]
fn index() -> &'static str {
    "Use https://challenge.hsiao.dev/01/<name>/<pass>"
}

fn main() {
    let mut eligible = HashSet::new();
    eligible.insert("alex");
    eligible.insert("brian");
    eligible.insert("dean");
    eligible.insert("demo");
    eligible.insert("lily");
    eligible.insert("lio");
    eligible.insert("lise");
    eligible.insert("luke");
    eligible.insert("myriam");
    eligible.insert("sunny");
    eligible.insert("timmy");

    let mut solved = HashSet::new();
    solved.insert("luke".to_string());
    solved.insert("demo".to_string());
    solved.insert("sunny".to_string());

    rocket::ignite()
        .manage(HitCount {
            total_hits: AtomicUsize::new(0),
            hits: Mutex::new(HashMap::new()),
            eligible: Mutex::new(eligible),
            solved: Mutex::new(solved),
            success_count: AtomicUsize::new(0),
        })
        .mount("/", routes![index, total, status, check])
        .mount("/", StaticFiles::from("data/"))
        .launch();
}
