use core::{fmt, panic};
use std::{
    env::ArgsOs,
    ffi::OsString,
    fs,
    io::{self, stdin, Read, StdoutLock, Write},
    iter::Peekable,
    process::ExitCode,
    vec,
};

#[derive(Debug, PartialEq, Eq)]
struct Params {
    file1: OsString,
    file2: OsString,
}

#[derive(Debug, PartialEq, Eq)]
enum ParseErr {
    InsufficientArgs,
}

impl fmt::Display for ParseErr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParseErr::InsufficientArgs => write!(f, "Insufficient args passed"),
        }
    }
}

impl std::error::Error for ParseErr {}

// Exit codes are documented at
// https://www.gnu.org/software/diffutils/manual/html_node/Invoking-sdiff.html.
//     An exit status of 0 means no differences were found,
//     1 means some differences were found,
//     and 2 means trouble.
pub fn main(opts: Peekable<ArgsOs>) -> ExitCode {
    let Ok(params) = parse_params(opts) else {
        // if we have insufficient args ...
        eprintln!("Usage: <exe> <file1> <file2>");
        return ExitCode::from(2);
    };

    // first we need to get the properly files
    let file1 = read_file_contents(&params.file1);
    let file2 = read_file_contents(&params.file2);

    // now we get the lines from the files as bytes, cuz the sdiff
    // must be compatible with ut8, ascii etc.
    let mut lines_left: Vec<&[u8]> = file1.split(|&c| c == b'\n').collect();
    let mut lines_right: Vec<&[u8]> = file2.split(|&c| c == b'\n').collect();

    // for some reason, the original file appends a empty line at
    // the end of file. I did not search for it, but my guess is
    // that this is EOL or an zeroed terminated file. Just remove it
    if lines_left.last() == Some(&&b""[..]) {
        lines_left.pop();
    }

    if lines_right.last() == Some(&&b""[..]) {
        lines_right.pop();
    }

    let width = 60;
    let max_lines = lines_left.len().max(lines_right.len());

    fn write_line(
        out: &mut StdoutLock,
        left: &[u8],
        right: &[u8],
        middle: &[u8],
        width: usize,
    ) -> io::Result<()> {
        let count = out.write(left.get(..width).unwrap_or(left))?;
        write!(out, "{}", " ".repeat(width - count))?;
        out.write(middle)?;
        out.write(right.get(..width).unwrap_or(right))?;
        Ok(())
    }

    let mut out = io::stdout().lock();
    for result in diff::slice(&lines_left, &lines_right) {
        match result {
            diff::Result::Left(str) => {
                write_line(&mut out, str, &[], b" < ", width).unwrap();
            }
            diff::Result::Right(str) => {
                write_line(&mut out, &[], &str, b" > ", width).unwrap();
            }
            diff::Result::Both(str_l, str_r) => {
                write_line(&mut out, str_l, str_r, b"   ", width).unwrap();
            }
        }
        writeln!(&mut out).unwrap();
    }

    ExitCode::SUCCESS
}

fn parse_params<I: Iterator<Item = OsString>>(mut opts: Peekable<I>) -> Result<Params, ParseErr> {
    opts.next(); // this is the executable name, just jmp it

    let Some(arg1) = opts.next() else {
        return Err(ParseErr::InsufficientArgs);
    };
    let Some(arg2) = opts.next() else {
        return Err(ParseErr::InsufficientArgs);
    };

    Ok(Params {
        file1: arg1,
        file2: arg2,
    })
}

fn read_file_contents(filepath: &OsString) -> Vec<u8> {
    if filepath == "-" {
        get_file_from_stdin()
    } else {
        fs::read(filepath).unwrap()
    }
}

fn get_file_from_stdin() -> Vec<u8> {
    let mut stdin = stdin().lock();
    let mut buf: Vec<u8> = vec![];

    if let Ok(_) = stdin.read_to_end(&mut buf) {
        return buf;
    } else {
        panic!("Failed to read from stdin")
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;

    use crate::sdiff::{parse_params, Params, ParseErr};

    fn str_os(str: &str) -> OsString {
        OsString::from(str)
    }

    #[test]
    fn test_params_convert() {
        assert_eq!(
            Ok(Params {
                file1: str_os("file1"),
                file2: str_os("file2")
            }),
            parse_params(
                [str_os("file1"), str_os("file2")]
                    .iter()
                    .cloned()
                    .peekable()
            )
        );
    }

    #[test]
    fn parse_params_returns_err_insufficient_args_when_opts_iter_has_not_even_one_item() {
        assert_eq!(
            Err(ParseErr::InsufficientArgs),
            parse_params([].iter().cloned().peekable())
        )
    }
}
