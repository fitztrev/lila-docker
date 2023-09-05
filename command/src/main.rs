use std::process::Command;

use cliclack::{confirm, input, intro, multiselect};
use git2::Repository;

const BANNER: &str = r#"
   |\_    _ _      _
   /o \  | (_) ___| |__   ___  ___ ___   ___  _ __ __ _
 (_. ||  | | |/ __| '_ \ / _ \/ __/ __| / _ \| '__/ _` |
   /__\  | | | (__| | | |  __/\__ \__ \| (_) | | | (_| |
  )___(  |_|_|\___|_| |_|\___||___/___(_)___/|_|  \__, |
                                                   |___/
"#;

const LICHESS_REPOS: [&str; 13] = [
    "lila",
    "lila-ws",
    "lila-db-seed",
    "lila-engine",
    "lila-fishnet",
    "lila-gif",
    "lila-search",
    "lifat",
    "scalachess",
    "api",
    "pgn-viewer",
    "chessground",
    "berserk",
];

fn main() -> std::io::Result<()> {
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 2 {
        println!("Usage: lila-docker <start|stop|down|resume>");
        return Ok(());
    }

    match args[1].as_str() {
        "start" => start()?,
        _ => println!("Invalid command"),
    }

    Ok(())
}

fn start() -> std::io::Result<()> {
    intro(BANNER)?;

    let profiles = multiselect("Select which optional services to run")
        .required(false)
        .item(
            "stockfish-play",
            "Stockfish (for playing against the computer)",
            "",
        )
        .item(
            "stockfish-analysis",
            "Stockfish (for requesting computer analysis of games)",
            "",
        )
        .item(
            "external-engine",
            "External Engine (for connecting a local chess engine to the analysis board)",
            "",
        )
        .item(
            "search",
            "Search (for searching games, forum posts, etc)",
            "",
        )
        .item("gifs", "GIFs (for generating animated GIFs of games)", "")
        .item("thumbnails", "Thumbnailer (for resizing images)", "")
        .interact()?;

    let setup_database = confirm("Do you want to seed the database with test users, games, etc?")
        .initial_value(true)
        .interact()?;

    let (_su_password, _password) = if setup_database {
        (
            input("Choose a password for admin users (blank for 'password')")
                .placeholder("password")
                .default_input("password")
                .required(false)
                .interact()?,
            input("Choose a password for regular users (blank for 'password')")
                .placeholder("password")
                .default_input("password")
                .required(false)
                .interact()?,
        )
    } else {
        (String::from(""), String::from(""))
    };

    for repo in LICHESS_REPOS.iter() {
        let repo_url = format!("https://github.com/lichess-org/{}.git", repo);
        Repository::clone(
            repo_url.as_str(),
            format!("/home/trevor/code/lila-docker/repos/{}", repo),
        )
        .ok();
    }

    cliclack::log::remark("Building Docker images...")?;
    let mut compose = Command::new("docker");
    compose.arg("compose");
    for profile in profiles.iter() {
        compose.arg("--profile").arg(profile);
    }
    match compose.arg("build").status() {
        Ok(_) => println!("Successfully built images"),
        Err(_) => println!("Failed to build images"),
    }

    cliclack::log::remark("Compiling lila js/css...")?;
    match Command::new("docker")
        .arg("compose")
        .arg("run")
        .arg("--rm")
        .arg("ui")
        .arg("bash")
        .arg("-c")
        .arg("/lila/ui/build")
        .status()
    {
        Ok(_) => println!("Successfully built UI"),
        Err(_) => println!("Failed to build UI"),
    }

    cliclack::log::remark("Compiling chessground...")?;
    match Command::new("docker")
        .arg("compose")
        .arg("run")
        .arg("--rm")
        .arg("ui")
        .arg("bash")
        .arg("-c")
        .arg("cd /chessground && pnpm install && pnpm run compile")
        .status()
    {
        Ok(_) => println!("Successfully built chessground"),
        Err(_) => println!("Failed to build chessground"),
    }

    cliclack::log::remark("Starting services...")?;
    let mut compose = Command::new("docker");
    compose.arg("compose");
    for profile in profiles.iter() {
        compose.arg("--profile").arg(profile);
    }
    match compose.arg("up").arg("-d").status() {
        Ok(_) => println!("Successfully started services"),
        Err(_) => println!("Failed to start services"),
    }

    if setup_database {
        // setup database
    }

    Ok(())
}
