use clap::Parser;
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use std::io::Read;
use std::process::Stdio;
use std::sync::mpsc::channel;
use std::thread;
use std::time::Duration;

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    #[clap(
        short,
        long,
        default_value_t = 1.0,
        help = "The delay between checks of whether the file has changed"
    )]
    delay: f32,
    #[clap(
        short,
        long,
        help = "Show the output of the main command and --also-run command"
    )]
    show_output: bool,
    #[clap(
        short,
        long,
        help = "Command that will be run for the entire duration of the program"
    )]
    also_run: Option<String>,
    #[clap(
        long,
        help = "Include debug information to view how a command is interpreted"
    )]
    debug: bool,
    #[clap(help = "File to watch")]
    filename: String,
    #[clap(help = "Command that will be run each time the file changes")]
    command: String,
}

fn watch(args: Args) -> notify::Result<()> {
    let command = args.command.clone();
    let mut command = command.split(" ");
    let mut cmd = std::process::Command::new(command.next().unwrap());
    cmd.stderr(Stdio::piped())
        .stdout(Stdio::piped())
        .args(command);

    if let Some(command) = args.also_run {
        let mut command = command.split(" ");
        let mut cmd = std::process::Command::new(command.next().unwrap());
        cmd.stderr(Stdio::piped())
            .stdout(Stdio::piped())
            .args(command);

        if args.debug {
            println!("DEBUG also run command={:?}", cmd);
        }

        thread::spawn(move || {
            let mut child = cmd.spawn().expect("unable to spawn secondary command");

            loop {
                match child.try_wait() {
                    Ok(Some(status)) => {
                        if !status.success() {
                            let mut buffer = String::new();
                            child
                                .stderr
                                .take()
                                .unwrap()
                                .read_to_string(&mut buffer)
                                .expect("Could not read child stderr");

                            println!(
                                "Processed exited with an error({}): {}",
                                status.code().unwrap(),
                                buffer
                            );
                        }

                        break;
                    }
                    Ok(None) => {
                        if args.show_output {
                            let mut buffer = String::new();
                            child
                                .stdout
                                .take()
                                .unwrap()
                                .read_to_string(&mut buffer)
                                .expect("Could not read child stderr");

                            println!("{}", buffer);
                        }
                    }
                    Err(e) => {
                        println!("Error attempting to wait for process: {e}");
                        break;
                    }
                }
            }
        });
    }

    if args.debug {
        println!("DEBUG command={:?}", cmd);
    }

    let (tx, rx) = channel();
    let mut watcher: RecommendedWatcher = Watcher::new(tx, Duration::from_secs_f32(args.delay))?;
    watcher.watch(args.filename, RecursiveMode::Recursive)?;

    loop {
        match rx.recv() {
            Ok(notify::DebouncedEvent::Write(_)) => {
                let output = cmd.output().unwrap();
                if !output.status.success() {
                    println!(
                        "Error occurred when running command \"{}\"\nError({}): {}",
                        args.command,
                        output.status.code().unwrap(),
                        String::from_utf8(output.stderr).unwrap()
                    );
                    std::process::exit(1);
                } else if args.show_output {
                    print!("{}", String::from_utf8(output.stdout).unwrap());
                }
            }
            Err(e) => println!("watch error: {:?}", e),
            _ => {}
        }
    }
}

fn main() {
    let mut args = Args::parse();
    args.command = args
        .command
        .replace("{}", &args.filename)
        .replace("{{", "{")
        .replace("}}", "}");

    if let Some(command) = args.also_run {
        args.also_run = Some(
            command
                .replace("{}", &args.filename)
                .replace("{{", "{")
                .replace("}}", "}"),
        );
    }

    if let Err(e) = watch(args) {
        println!("error: {:?}", e);
    }
}
