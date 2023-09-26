use std::{path::PathBuf, process::Command};

use std::io::Error;

use cliclack::{confirm, input, intro, log, multiselect, spinner};
use git2::Repository;
use home::home_dir;
use serde::{Deserialize, Serialize};
use strum::{EnumIter, EnumString, IntoEnumIterator};

const BANNER: &str = r"
   |\_    _ _      _
   /o \  | (_) ___| |__   ___  ___ ___   ___  _ __ __ _
 (_. ||  | | |/ __| '_ \ / _ \/ __/ __| / _ \| '__/ _` |
   /__\  | | | (__| | | |  __/\__ \__ \| (_) | | | (_| |
  )___(  |_|_|\___|_| |_|\___||___/___(_)___/|_|  \__, |
                                                   |___/
";

#[derive(Debug, Serialize, Deserialize)]
struct Config {
    repos_dir: String,
    profiles: Vec<String>,
    setup_database: bool,
    su_password: String,
    password: String,
}

const LICHESS_REPOS: [&str; 13] = [
    "lichess-org/lila",
    "lichess-org/lila-ws",
    "lichess-org/lila-db-seed",
    "lichess-org/lila-engine",
    "lichess-org/lila-fishnet",
    "lichess-org/lila-gif",
    "lichess-org/lila-search",
    "lichess-org/lifat",
    "lichess-org/scalachess",
    "lichess-org/api",
    "lichess-org/pgn-viewer",
    "lichess-org/chessground",
    "lichess-org/berserk",
];

fn path_to_config_file() -> PathBuf {
    home_dir().unwrap().join(".lila-docker")
}

#[derive(Default, Clone, Eq, PartialEq, Debug)]
struct OptionalService {
    compose_profile: Option<ComposeProfile>,
    repositories: Option<Vec<Repository>>,
}

#[derive(Debug, Clone, PartialEq, EnumString, strum::Display, Eq, EnumIter)]
#[strum(serialize_all = "kebab-case")]
enum ComposeProfile {
    StockfishPlay,
    StockfishAnalysis,
    ExternalEngine,
    Search,
    Gifs,
    Thumbnails,
    ApiDocs,
    Chessground,
    PgnViewer,
}

#[derive(Debug, Clone, PartialEq, EnumString, strum::Display, Eq, EnumIter)]
#[strum(serialize_all = "kebab-case")]
enum Repository {
    Lila,
    LilaWs,
    LilaDbSeed,
    Lifat,
    LilaFishnet,
    LilaEngine,
    LilaSearch,
    LilaGif,
    Api,
    Chessground,
    PgnViewer,
    Scalachess,
    Dartchess,
    Berserk,
    #[strum(serialize = "cyanfish/bbpPairings")]
    BbpPairings,
}

fn main() -> std::io::Result<()> {
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 2 {
        intro(BANNER)?;
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

    let services = prompt_for_optional_services()?;

    let setup_database =
        confirm("Do you want to seed the database with test users, games, etc? (Recommended)")
            .initial_value(true)
            .interact()?;

    let (su_password, password) = if setup_database {
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

    let default_repos_location = home_dir().unwrap().join("lila-docker");
    let repos_dir: String = input("Where do you want the repos cloned?")
        .placeholder(default_repos_location.to_str().unwrap())
        .default_input(default_repos_location.to_str().unwrap())
        .required(true)
        .interact()?;

    let config = Config {
        repos_dir,
        profiles: profiles.iter().map(|s| s.to_string()).collect(),
        setup_database,
        su_password,
        password,
    };

    let contents = toml::to_string(&config).unwrap();
    std::fs::write(path_to_config_file(), contents)?;

    log::success("Wrote config file to ~/.lila-docker")?;

    for repo in LICHESS_REPOS.iter() {
        let repo_url = format!("https://github.com/{}.git", repo);

        let mut progress = spinner();
        progress.start(format!("Cloning {}...", repo));
        Repository::clone(
            repo_url.as_str(),
            format!("{}/{}", config.repos_dir, repo).as_str(),
        )
        .ok();
        progress.stop(format!("Cloned {}", repo));
    }

    log::info("Initializing submodules...")?;
    let mut submodule = Command::new("git");
    submodule
        .arg("-C")
        .arg(format!("{}/lichess-org/lila", config.repos_dir))
        .arg("submodule")
        .arg("update")
        .arg("--init");
    match submodule.status() {
        Ok(_) => log::success("Initialized submodules")?,
        Err(_) => log::error("Failed to initialize submodules")?,
    }

    log::info("Building Docker images...")?;
    let mut compose = Command::new("docker");
    compose.arg("compose");
    for profile in profiles.iter() {
        compose.arg("--profile").arg(profile);
    }
    match compose.arg("build").status() {
        Ok(_) => log::success("Built Docker images")?,
        Err(_) => log::error("Failed to build Docker images")?,
    }

    log::info("Compiling lila js/css...")?;
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
        Ok(_) => log::success("Successfully built UI")?,
        Err(_) => log::error("Failed to build UI")?,
    }

    // let parsed = toml::from_str::<Config>(&contents).unwrap();
    // println!("parsed: {:?}", parsed);

    Ok(())
}

fn prompt_for_optional_services() -> Result<Vec<OptionalService>, Error> {
    multiselect(
        "Select which optional services to include:\n    (Use arrows, <space> to toggle, <enter> to continue)\n",
    )
    .required(false)
    .item(
        OptionalService {
            compose_profile: Some(ComposeProfile::StockfishPlay),
            repositories: vec![Repository::LilaFishnet].into(),
        },
        "Stockfish Play",
        "for playing against the computer",
    )
    .item(
        OptionalService {
            compose_profile: Some(ComposeProfile::StockfishAnalysis),
            repositories: None,
        },
        "Stockfish Analysis",
        "for requesting computer analysis of games",
    )
    .item(
        OptionalService {
            compose_profile: Some(ComposeProfile::ExternalEngine),
            repositories: vec![Repository::LilaEngine].into(),
        },
        "External Engine",
        "for connecting a local chess engine to the analysis board",
    )
    .item(
        OptionalService {
            compose_profile: Some(ComposeProfile::Search),
            repositories: vec![Repository::LilaSearch].into(),
        },
        "Search",
        "for searching games, forum posts, etc",
    )
    .item(
        OptionalService {
            compose_profile: Some(ComposeProfile::Gifs),
            repositories: vec![Repository::LilaGif].into(),
        },
        "GIFs",
        "for generating animated GIFs of games",
    )
    .item(
        OptionalService {
            compose_profile: Some(ComposeProfile::Thumbnails),
            repositories: None,
        },
        "Thumbnail generator",
        "for resizing blog/streamer images",
    )
    .item(
        OptionalService {
            compose_profile: Some(ComposeProfile::ApiDocs),
            repositories: vec![Repository::Api].into(),
        },
        "API docs",
        "standalone API documentation",
    )
    .item(
        OptionalService {
            compose_profile: Some(ComposeProfile::Chessground),
            repositories: vec![Repository::Chessground].into(),
        },
        "Chessground",
        "standalone board UI",
    )
    .item(
        OptionalService {
            compose_profile: Some(ComposeProfile::PgnViewer),
            repositories: vec![Repository::PgnViewer].into(),
        },
        "PGN Viewer",
        "standalone PGN viewer",
    )
    .item(
        OptionalService {
            compose_profile: None,
            repositories: vec![Repository::Scalachess].into(),
        },
        "Scalachess",
        "standalone chess logic library",
    )
    .item(
        OptionalService {
            compose_profile: None,
            repositories: vec![Repository::Dartchess].into(),
        },
        "Dartchess",
        "standalone chess library for mobile platforms",
    )
    .item(
        OptionalService {
            compose_profile: None,
            repositories: vec![Repository::Berserk].into(),
        },
        "Berserk",
        "Python API client",
    )
    .item(
        OptionalService {
            compose_profile: None,
            repositories: vec![Repository::BbpPairings].into(),
        },
        "Swiss Pairings",
        "bbpPairings tool",
    )
    .interact()
}
