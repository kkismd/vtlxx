use std::{env, ffi::OsString, fmt, fs, io, process::ExitCode};
use vtlxx::{Machine, SourceError};

#[derive(Debug)]
enum CliError {
    Usage,
    Input(io::Error),
    Source(SourceError),
    SeedRange,
}

impl fmt::Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Usage => write!(f, "usage: vtlxx <source-file> [--seed N]"),
            Self::Input(error) => write!(f, "cannot read source file: {error}"),
            Self::Source(error) => write!(f, "VTLXX error: {error:?}"),
            Self::SeedRange => write!(f, "--seed must be a signed 16-bit decimal integer"),
        }
    }
}

fn arguments() -> Result<(OsString, Option<i16>), CliError> {
    let mut args = env::args_os();
    args.next();
    let path = args.next().ok_or(CliError::Usage)?;
    match (args.next(), args.next(), args.next()) {
        (None, None, None) => Ok((path, None)),
        (Some(flag), Some(value), None) if flag == "--seed" => {
            let value = value.to_str().ok_or(CliError::SeedRange)?;
            let digits = value
                .strip_prefix('-')
                .or_else(|| value.strip_prefix('+'))
                .unwrap_or(value);
            if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
                return Err(CliError::SeedRange);
            }
            let seed = value.parse::<i16>().map_err(|_| CliError::SeedRange)?;
            Ok((path, Some(seed)))
        }
        _ => Err(CliError::Usage),
    }
}

fn run() -> Result<(), CliError> {
    let (path, seed) = arguments()?;
    let source = fs::read_to_string(path).map_err(CliError::Input)?;
    let mut machine = Machine::new();
    if let Some(seed) = seed {
        machine.set_entropy_source(move || Ok(seed));
    }
    machine.set_input(io::stdin());
    machine.set_output(io::stdout());
    machine.execute_source(&source).map_err(CliError::Source)
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
