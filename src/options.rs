use crate::manager::Manager;
use regex::Regex;

fn print_usage(dollar0: &str, opts: getopts::Options) {
    let brief = format!("Usage: {} [options] PROGRAM [root_dir]", dollar0);
    println!("{}", opts.usage(&brief));
    println!("PROGRAM will be run in parallel in every subdirectory (SUB), as SUB's owner.");
    println!("A placeholder \"{{}}\" is available to PROGRAM, it will be replaced with SUB.");
}

pub struct Options {
    pub sock_path: String,
    pub program: String,
    pub root_dir: String,
    pub watch_ignore: Option<Regex>,
    pub management: super::Manager,
}

pub fn get_options() -> Options {
    let args: Vec<String> = std::env::args().collect();
    let mut opts = getopts::Options::new();

    opts.optopt(
        "t",
        "type",
        "set the management type [choices: watch, socket, none] [default: none]",
        "TYPE",
    );

    opts.optopt(
        "s",
        "socket",
        "set the socket path. sending the socket a message like \"restart xxx\" will restart the process running in the directory \"xxx\". [default: ./subsocket]",
        "NAME",
    );

    opts.optopt(
        "i",
        "watch-ignore",
        "pattern to ignore when watching (matches whole path)",
        "PATTERN",
    );

    opts.optflag("h", "help", "get help");

    let matches = match opts.parse(&args[1..]) {
        Ok(m) => m,
        Err(f) => panic!(f.to_string()),
    };

    let program = matches.free.get(0);

    let management = match matches.opt_str("t") {
        Some(string) => match string.as_ref() {
            "watch" => Some(Manager::Watch),
            "socket" | "sock" => Some(Manager::Socket),
            "none" => Some(Manager::None),
            _ => None,
        },
        None => Some(Manager::None),
    };

    let should_show_help = matches.opt_present("h") || program.is_none() || management.is_none();

    if should_show_help {
        print_usage(&args[0], opts);
        std::process::exit(1);
    }

    let default_root_dir = ".".to_string();
    let root_dir = matches.free.get(1).unwrap_or(&default_root_dir).to_string();

    let sock_path = matches
        .opt_str("s")
        .unwrap_or_else(|| "subsocket".to_string());

    let watch_ignore = match matches.opt_str("i") {
        Some(string) => Some(Regex::new(&string).expect("-i was not a valid regex")),
        None => None,
    };

    Options {
        program: program.unwrap().to_string(),
        watch_ignore,
        sock_path,
        root_dir,
        management: management.unwrap(),
    }
}
