use std::collections::HashSet;
use std::{
    io::{ErrorKind, Result}, 
    fs::{OpenOptions, rename}
};
use std::path::PathBuf;
use std::env;

use clap::{Parser, Subcommand};

#[derive(Debug, Clone)]
struct CsvIndexError;

impl std::fmt::Display for CsvIndexError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "incorrect index found")
    }
}

impl std::error::Error for CsvIndexError {}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
struct Record {
    index: u32,
    action: String,
    done: bool,
}

impl std::fmt::Display for Record {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} {} {}", self.index, self.action, if self.done {"Y"} else {"X"})
    }
}

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Optional name to operate on
    name: Option<String>,

    /// Sets a custom list file
    #[arg(short, long, value_name = "FILE")]
    file: Option<PathBuf>,

    /// Turn debugging information on
    #[arg(short, long, action = clap::ArgAction::Count)]
    debug: u8,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// does testing things
    Test {
        /// lists test values
        #[arg(short, long)]
        list: bool,
    },

    /// adds to the list
    Add {
        #[arg(action = clap::ArgAction::Append)]
        names: Vec<String>,
    },

    /// removes indexes from the list
    Rm {
        #[arg(action = clap::ArgAction::Append)]
        indexes: Vec<u32>,
    },

    /// marks indexes as done
    Done {
        #[arg(action = clap::ArgAction::Append)]
        indexes: Vec<u32>,

        /// set all to done
        #[arg(short)]
        all: bool
    },

    /// marks indexes as not done
    Undo {
        #[arg(action = clap::ArgAction::Append)]
        indexes: Vec<u32>,
        
        /// set all to not done
        #[arg(short)]
        all: bool
    },

    Reset,
}

fn no_header_reader() -> csv::ReaderBuilder {
    let mut res = csv::ReaderBuilder::new();
    res.has_headers(false);
    return res;
}

fn no_header_writer() -> csv::WriterBuilder {
    let mut res = csv::WriterBuilder::new();
    res.has_headers(false);
    return res;
}

fn list_todos(path: PathBuf) -> Result<()> {
    let mut rdr = no_header_reader().from_path(path)?;
    for result in rdr.deserialize() {
        let record: Record = result?;
        println!("{}", record);
    }

    
    return Ok(());
}

fn add_records(path: PathBuf, actions: &Vec<String>, i: u32) -> Result<()> {
    let mut writer = no_header_writer()
        .from_writer(OpenOptions::new()
            .write(true)
            .append(true)
            .open(path)?
        );
    let mut last = i;
    for action in actions {
        writer.serialize(Record {
            index: last,
            action: action.to_string(),
            done: false,
        })?;
        last += 1;
    }

    return Ok(());
}


type Rp = fn(u32, &Record) -> Option<Record>;

fn file_map(path: &PathBuf, indexes: &mut HashSet<u32>, all: bool, f: Rp) -> Result<()> {
    let mut i: u32 = 1;
    let mut aux: PathBuf = path.iter().collect();
    aux.pop();
    aux.push("aux.csv");
    let mut rdr = no_header_reader().from_path(path)?;
    let mut writer = no_header_writer()
        .from_writer(OpenOptions::new()
            .create(true)
            .write(true)
            .open(&aux)?
        );
    for result in rdr.deserialize() {
        let record: Record = result?;
        if all || indexes.remove(&record.index) {
            if let Some(rec) = f(i, &record) {
                writer.serialize(rec)?;
            } else {
                continue;
            }
        } else {
            writer.serialize(Record {
                index: i,
                action: record.action,
                done: record.done,
            })?;
        }
        i += 1;
    }
    rename(aux.as_path(), path.as_path())?;

    return Ok(());
}

fn rm_records(path: &PathBuf, indexes: &mut HashSet<u32>) -> Result<()> {
    return 
        file_map(path, indexes, false, 
        |_, _| None, 
        );
}


fn mark_records(path: &PathBuf, indexes: &mut HashSet<u32>, all: bool) -> Result<()> {
    return 
        file_map(path, indexes, all, 
        |i, r| Some(Record { index: i, action: r.action.clone(), done: true}), 
        );
}

fn unmark_records(path: &PathBuf, indexes: &mut HashSet<u32>, all: bool) -> Result<()> {
    return 
        file_map(path, indexes, all, 
        |i, r| Some(Record { index: i, action: r.action.clone(), done: false}), 
        );
}

fn reset_records(path: &PathBuf) -> Result<()> {
    return 
        file_map(path, &mut HashSet::from_iter(vec![1]), true, 
        |_, _| None, 
        );
}

fn check_file_get_last(path: &PathBuf) -> Result<u32> {
    if !path.exists() {
        std::fs::File::create(path)?;
        return Ok(1);
    }
    let mut i = 1;
    let mut rdr = no_header_reader().from_path(path)?;
    for result in rdr.deserialize() {
        let record: Record = result?;
        if i != record.index {
            return Err(std::io::Error::new(ErrorKind::Other, CsvIndexError));
        }
        i += 1;
    }
    return Ok(i);
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // You can check the value provided by positional arguments, or option arguments
    if let Some(name) = cli.name.as_deref() {
        println!("Value for name: {name}");
    }

    let path = match cli.file {
        Some(path)  => path,
        None        => {
            let mut path = PathBuf::from(env::var_os("HOME").unwrap());
            path.push("todo_list.csv");
            path
        }
    };

    let last_index = check_file_get_last(&path)?;

    if let None = &cli.command {
        return list_todos(path);
    } 

    return match &cli.command.unwrap() {
        Commands::Add { names } => add_records(path, names, last_index),
        Commands::Rm { indexes } => {
            rm_records(&path, &mut HashSet::from_iter(indexes.iter().cloned()), )
        },
        Commands::Done { indexes, all } => {
            mark_records(&path, &mut HashSet::from_iter(indexes.iter().cloned()), *all)
        },
        Commands::Undo { indexes, all } => {
            unmark_records(&path, &mut HashSet::from_iter(indexes.iter().cloned()), *all)
        }
        Commands::Reset => {
            reset_records(&path)
        }
        _ => Ok(()),

    }
}
