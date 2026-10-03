use std::{env, ffi::OsString, fmt, fs, io, io::Write, process::ExitCode};
use vtlxx_poc_extended_classic_vtl::{Machine, SourceError};

#[derive(Debug)]
enum CliError {
    Usage,
    Input(io::Error),
    Source(SourceError),
    Output(io::Error),
}

impl fmt::Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Usage => write!(f, "usage: ecvtl <source-file>"),
            Self::Input(error) => write!(f, "cannot read source file: {error}"),
            Self::Source(error) => write!(f, "ECVTL error: {error:?}"),
            Self::Output(error) => write!(f, "cannot write output: {error}"),
        }
    }
}

fn source_path() -> Result<OsString, CliError> {
    let mut args = env::args_os();
    args.next();
    let path = args.next().ok_or(CliError::Usage)?;
    if args.next().is_some() {
        return Err(CliError::Usage);
    }
    Ok(path)
}

fn run() -> Result<(), CliError> {
    let path = source_path()?;
    let source = fs::read_to_string(path).map_err(CliError::Input)?;
    let mut machine = Machine::new();
    machine.execute_source(&source).map_err(CliError::Source)?;
    io::stdout()
        .lock()
        .write_all(machine.output())
        .map_err(CliError::Output)
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}
