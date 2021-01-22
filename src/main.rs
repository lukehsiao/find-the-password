#![feature(proc_macro_hygiene, decl_macro)]
#[macro_use]
extern crate rocket;

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

use chrono::Utc;
use data_encoding::HEXLOWER;
use rocket::State;
use rocket_contrib::serve::StaticFiles;
use sha2::{Digest, Sha224};

#[derive(Debug)]
struct HitCount {
    total_hits: AtomicUsize,
    hits: Mutex<HashMap<String, usize>>,
    eligible: Mutex<HashSet<&'static str>>,
    success_count: AtomicUsize,
}

const PASS: &str = "ee21c52cba80a3b9bb1e237c3c84166f";

#[get("/01/<name>/<pass>")]
fn check(name: String, pass: String, count: State<HitCount>) -> String {
    count.total_hits.fetch_add(1, Ordering::Relaxed);
    *count.hits.lock().unwrap().entry(name.clone()).or_insert(0) += 1;
    let attempts = count.hits.lock().unwrap().get(&name).unwrap().clone();

    let mut eligible = count.eligible.lock().unwrap();

    if eligible.contains(name.as_str()) {
        if pass.as_str() == PASS {
            count.success_count.fetch_add(1, Ordering::Relaxed);
            let success_count = count.success_count.load(Ordering::Relaxed);
            eligible.remove(name.as_str());

            let mut hasher = Sha224::new();
            hasher.update(format!("{}_{}\n", name, success_count));
            let result = hasher.finalize();

            eprintln!(
                "[SUCCESS] {} got {} place at {}",
                name,
                success_count,
                Utc::now().to_rfc3339()
            );

            format!(
                "Yes\n\n{}, you solved it!\n{} was correct!\nYou got {} place after {} attempts!\nSend this code to Luke to redeem your prize: {}",
                name,
                pass,
                count.success_count.load(Ordering::Relaxed),
                attempts,
                HEXLOWER.encode(&result[..]),
            )
        } else {
            format!(
                "No\n\nHello, {}!\n{} is incorrect.\nYou've tried {} times.",
                name.as_str(),
                pass.as_str(),
                attempts,
            )
        }
    } else {
        format!(
            "{}, you're not eligible to try this challenge.\nDid you already solve it?\nAre you using the right name?",
            name
        )
    }
}

#[get("/")]
fn index() -> &'static str {
    "Use challenge.hsiao.dev/01/<name>/<pass>"
}

fn main() {
    let mut eligible = HashSet::new();
    eligible.insert("alex");
    eligible.insert("brian");
    eligible.insert("dean");
    eligible.insert("leo");
    eligible.insert("lily");
    eligible.insert("lise");
    eligible.insert("luke");
    eligible.insert("myriam");
    eligible.insert("sunny");
    eligible.insert("timmy");

    rocket::ignite()
        .manage(HitCount {
            total_hits: AtomicUsize::new(0),
            hits: Mutex::new(HashMap::new()),
            eligible: Mutex::new(eligible),
            success_count: AtomicUsize::new(0),
        })
        .mount("/", routes![index, check])
        .mount("/", StaticFiles::from("data/passwords.txt"))
        .launch();
}
