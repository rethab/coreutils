#![crate_name = "uu_env"]
/*
 * This file is part of the uutils coreutils package.
 *
 * (c) Jordi Boggiano <j.boggiano@seld.be>
 *
 * For the full copyright and license information, please view the LICENSE
 * file that was distributed with this source code.
 */

/* last synced with: env (GNU coreutils) 8.13 */

#[macro_use]
extern crate uucore;

extern crate ini;

use ini::Ini;
use std::env;
use std::io::{stdin, stdout, Write};
use std::process::Command;

static NAME: &str = "env";
static SYNTAX: &str = "[OPTION]... [-] [NAME=VALUE]... [COMMAND [ARG]...]";
static SUMMARY: &str = "Set each NAME to VALUE in the environment and run COMMAND";
static LONG_HELP: &str = "
 A mere - implies -i. If no COMMAND, print the resulting environment
";

struct Options {
    ignore_env: bool,
    null: bool,
    files: Vec<String>,
    unsets: Vec<String>,
    sets: Vec<(String, String)>,
    program: Vec<String>,
}

// print name=value env pairs on screen
// if null is true, separate pairs with a \0, \n otherwise
fn print_env(null: bool) {
    for (n, v) in env::vars() {
        print!("{}={}{}", n, v, if null { '\0' } else { '\n' });
    }
}

fn split_string(s: &str) -> Vec<String> {
    s.split_whitespace().map(|x| x.to_owned()).collect::<Vec<String>>()
}

#[cfg(not(windows))]
fn build_command(mut args: Vec<String>) -> (String, Vec<String>) {
    (args.remove(0), args)
}

#[cfg(windows)]
fn build_command(mut args: Vec<String>) -> (String, Vec<String>) {
    args.insert(0, "/d/c".to_string());
    (env::var("ComSpec").unwrap_or("cmd".to_string()), args)
}

pub fn uumain(args: Vec<String>) -> i32 {
    let mut core_opts = new_coreopts!(SYNTAX, SUMMARY, LONG_HELP);
    core_opts
        .optflag("i", "ignore-environment", "start with an empty environment")
        .optflag(
            "0",
            "null",
            "end each output line with a 0 byte rather than newline (only valid when printing the environment)",
        )
        .optflag(
            "S",
            "split-string",
            "process and split S into separate arguments; used to pass multiple arguments on shebang lines"
        )
        .optopt("f", "file", "read and sets variables from the file (prior to sets/unsets)", "FILE")
        .optopt("u", "unset", "remove variable from the environment", "NAME");

    let mut opts = Box::new(Options {
        ignore_env: false,
        null: false,
        unsets: vec![],
        files: vec![],
        sets: vec![],
        program: vec![],
    });

    let mut wait_cmd = false;
    let mut iter = args.iter();
    iter.next(); // skip program
    let mut item = iter.next();

    // the for loop doesn't work here,
    // because we need sometimes to read 2 items forward,
    // and the iter can't be borrowed twice
    while item != None {
        let opt = item.unwrap();

        if wait_cmd {
            // we still accept NAME=VAL here but not other options
            let mut sp = opt.splitn(2, '=');
            let name = sp.next();
            let value = sp.next();

            match (name, value) {
                (Some(n), Some(v)) => {
                    opts.sets.push((n.to_owned(), v.to_owned()));
                }
                _ => {
                    // read the program now
                    opts.program.push(opt.to_owned());
                    break;
                }
            }
        } else if opt.starts_with("--") {
            match opt.as_ref() {
                "--help" => {
                    core_opts.parse(vec![String::new(), String::from("--help")]);
                    return 0;
                }
                "--version" => {
                    core_opts.parse(vec![String::new(), String::from("--version")]);
                    return 0;
                }

                "--ignore-environment" => opts.ignore_env = true,
                "--null" => opts.null = true,
                "--file" => {
                    let var = iter.next();

                    match var {
                        None => println!("{}: this option requires an argument: {}", NAME, opt),
                        Some(s) => opts.files.push(s.to_owned()),
                    }
                }
                "--unset" => {
                    let var = iter.next();

                    match var {
                        None => eprintln!("{}: this option requires an argument: {}", NAME, opt),
                        Some(s) => opts.unsets.push(s.to_owned()),
                    }
                }
                prefix if prefix.starts_with("--split-string") => {
                    let length = "--split-string".len();
                    if prefix.len() == length { // when used like "env --split-string 'foo bar'"
                        let string = iter.next();
                        match string {
                            None => eprintln!("{}: this option requires an argument: {}", NAME, opt),
                            Some(s) => opts.program.append(&mut split_string(s)) ,
                        }

                    } else { // everything is passed as one argument (typical for shebang)
                        opts.program.append(&mut split_string(opt[length..].trim()));
                    }

                }

                _ => {
                    eprintln!("{}: invalid option \"{}\"", NAME, *opt);
                    eprintln!("Type \"{} --help\" for detailed information", NAME);
                    return 1;
                }
            }
        } else if opt.starts_with("-") {
            if opt.len() == 1 {
                // implies -i and stop parsing opts
                wait_cmd = true;
                opts.ignore_env = true;
                continue;
            }

            // split string is handled separately, because it doesn't only operate on characters
            if opt.starts_with("-S") {
                if opt.len() == 2 { // when used like "env -S 'foo bar'"
                    let string = iter.next();
                    match string {
                        None => eprintln!("{}: this option requires an argument: {}", NAME, opt),
                        Some(s) => opts.program.append(&mut split_string(s)) ,
                    }

                } else { // everything is passed as one argument, typical for shebang
                    opts.program.append(&mut split_string(opt[2..].trim()));
                }

            } else {

                let mut chars = opt.chars();
                chars.next(); // consume dash

                for c in chars {
                    // short versions of options
                    match c {
                        'i' => opts.ignore_env = true,
                        '0' => opts.null = true,
                        'f' => {
                            let var = iter.next();

                            match var {
                                None => println!("{}: this option requires an argument: {}", NAME, opt),
                                Some(s) => opts.files.push(s.to_owned()),
                            }
                        }
                        'u' => {
                            let var = iter.next();

                            match var {
                                None => eprintln!("{}: this option requires an argument: {}", NAME, opt),
                                Some(s) => opts.unsets.push(s.to_owned()),
                            }
                        }
                        _ => {
                            eprintln!("{}: illegal option -- {}", NAME, c);
                            eprintln!("Type \"{} --help\" for detailed information", NAME);
                            return 1;
                        }
                    }
                }

            }

        } else {
            // is it a NAME=VALUE like opt ?
            let mut sp = opt.splitn(2, '=');
            let name = sp.next();
            let value = sp.next();

            match (name, value) {
                (Some(n), Some(v)) => {
                    // yes
                    opts.sets.push((n.to_owned(), v.to_owned()));
                    wait_cmd = true;
                }
                // no, its a program-like opt
                _ => {
                    if opts.null {
                        eprintln!("{}: cannot specify --null (-0) with command", NAME);
                        eprintln!("Type \"{} --help\" for detailed information", NAME);
                        return 1;
                    }
                    opts.program.push(opt.clone());
                    break;
                }
            }
        }

        item = iter.next();
    }

    // read program arguments
    for opt in iter {
        if opts.null {
            eprintln!("{}: cannot specify --null (-0) with command", NAME);
            eprintln!("Type \"{} --help\" for detailed information", NAME);
            return 1;
        }
        opts.program.push(opt.clone())
    }

    if opts.ignore_env {
        for (ref name, _) in env::vars() {
            env::remove_var(name);
        }
    }

    for file in &opts.files {
        let conf = if file == "-" {
            let stdin = stdin();
            let mut stdin_locked = stdin.lock();
            Ini::read_from(&mut stdin_locked)
        } else {
            Ini::load_from_file(file)
        };
        let conf = match conf {
            Ok(config) => config,
            Err(error) => {
                eprintln!("env: error: \"{}\": {}", file, error);
                return 1;
            }
        };
        for (_, prop) in &conf {
            for (key, value) in prop {
                env::set_var(key, value);
            }
        }
    }

    for name in &opts.unsets {
        env::remove_var(name);
    }

    for &(ref name, ref val) in &opts.sets {
        env::set_var(name, val);
    }

    if !opts.program.is_empty() {
        let (prog, args) = build_command(opts.program);
        match Command::new(prog).args(args).status() {
            Ok(exit) => {
                return if exit.success() {
                    0
                } else {
                    exit.code().unwrap()
                }
            }
            Err(_) => return 1,
        }
    } else {
        // no program provided
        print_env(opts.null);
        return_if_err!(1, stdout().flush());
    }

    0
}
