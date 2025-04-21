use std::vec;

use crate::utils::limited_string;
use diff::Result;

type Buf = Vec<u8>;

#[derive(Debug, PartialEq)]
struct Line<'a> {
    line_ndx: usize,
    content: &'a [u8],
}

#[derive(Debug, PartialEq)]
struct Diff<'a> {
    left_ln: &'a Line<'a>,
    right_ln: &'a Line<'a>,
}

impl<'a> Diff<'a> {
    fn new(left_ln: &'a Line, right_ln: &'a Line) -> Diff<'a> {
        Diff { left_ln, right_ln }
    }
}

impl<'a> Line<'a> {
    pub fn new(line_ndx: usize, content: &'a [u8]) -> Self {
        Line { line_ndx, content }
    }
}

fn dispatch_to_output(
    output: &mut Buf,
    to_dispatch_val: &Diff,
    already_dispatched: &mut Vec<usize>,
) {
    if already_dispatched.contains(&to_dispatch_val.left_ln.line_ndx) {
        return;
    } else {
        fn push_output(
            output: &mut Buf,
            left_ln: &[u8],
            right_ln: &[u8],
            symbol: &[u8],
            tab_size: usize,
        ) {
            // The reason why this function exists, is that we cannot
            // assume a enconding for our left or right line, and the
            // writeln!() macro obligattes us to do it.

            // side-by-side diff usually prints the output like:
            // {left_line}{tab}{space_char}{symbol(|, < or >)}{space_char}{right_line}{EOL}

            // recalculate how many spaces are nescessary, cause we need to take into
            // consideration the lenght of the word before print it.
            let tab_size = (tab_size as isize - left_ln.len() as isize).max(0);

            left_ln.iter().for_each(|&b| output.push(b)); // {left_line}
            for _ in 0..(tab_size + 1)
            /*Just more one space where we are going to print the symbol */
            {
                output.push(b' '); // {tab} + {space_char}
            }
            symbol.iter().for_each(|&b| output.push(b)); // {symbol}
            output.push(b' '); // {space_char}
            right_ln.iter().for_each(|&b| output.push(b)); // {right_line}

            if cfg!(target_os = "windows") {
                // {EOL}
                output.push(b'\r');
                output.push(b'\n');
            } else {
                output.push(b'\n');
            }
        }

        let tab_spaces = 61;
        let limiter = tab_spaces; // for some reason the str goes only to 61 chars, not 60
        already_dispatched.push(to_dispatch_val.left_ln.line_ndx);
        if to_dispatch_val.right_ln.content != vec![] && to_dispatch_val.left_ln.content == vec![] {
            push_output(
                output,
                "".as_bytes(),
                &limited_string(&to_dispatch_val.right_ln.content, limiter),
                ">".as_bytes(),
                tab_spaces,
            );
        } else if to_dispatch_val.left_ln.content != vec![]
            && to_dispatch_val.right_ln.content == vec![]
        {
            push_output(
                output,
                &limited_string(&to_dispatch_val.left_ln.content, limiter),
                "".as_bytes(),
                "<".as_bytes(),
                tab_spaces,
            );
        } else {
            let symbol = if to_dispatch_val.left_ln.content == to_dispatch_val.right_ln.content {
                " "
            } else {
                "|"
            };

            push_output(
                output,
                &limited_string(&to_dispatch_val.left_ln.content, limiter),
                &limited_string(&to_dispatch_val.right_ln.content, limiter),
                symbol.as_bytes(),
                tab_spaces,
            );
        }
    }
}

pub fn diff(from_file: &Buf, to_file: &Buf) -> Buf {
    //      ^ The left file  ^ The right file
    fn split_lines(input: &[u8]) -> Vec<Line> {
        input
            .split(|&c| c == b'\n')
            .enumerate()
            .map(|(i, line)| Line::new(i, line))
            .collect()
    }

    // if from_file.is_empty() && to_file.is_empty() {
    //     return vec![];
    // }

    let mut already_dispatched = Vec::new();
    let mut output = Vec::new();
    let left_lines = split_lines(from_file);
    let right_lines = split_lines(to_file);

    // just saying that is impossible to have an empty buffer
    debug_assert_eq!(split_lines(&[]).len(), 1);

    // if we have an empty buffer at both sides, the tool will
    // compare them. There is no work here
    if left_lines[0].content == vec![] && right_lines[0].content == vec![] {
        return vec![];
    }

    // I want to make clear some things that the lib
    // diff does not explain clearly in the docs.
    // If we want to compare, let say this two sequences:
    // Sequence 1: """
    // 1. Apple
    // 2. Strawberry
    // 3. Potato
    // """
    // and
    // Sequence 2: """
    // 1. Apple
    // 2. Orange
    // 3. Potato
    // """
    // The lib will produce the result sequence:
    // Result sequence: """
    // [
    //  diff::Result::Both("1. Apple"),
    //  diff::Result::Left("2. Strawberry"),
    //  diff::Result::Right("2. Orange"),
    //  diff::Result::Both("3. Potato")
    // ]
    // """
    // The lib says that the slice method Computes the diff
    // between two slices., giving a little margin of
    // interpretation about how it works.
    // Also, the docs does not taste any
    // kind of examples about the produced result by the lib.

    for result in diff::slice(&left_lines, &right_lines) {
        match result {
            Result::Left(left_ln) => {
                // If we have a side match, that does not mean anything.
                // We have to check if we have a correspondent line
                // in the other file at the same index. If we have, dispatch
                // the first line and its correspondent line. Otherwise, dispatch
                // the present line along with an fake line with the same index

                let diff;
                let fake_line = Line::new(left_ln.line_ndx, &[]);
                let Some(right_ln) = right_lines.get(left_ln.line_ndx) else {
                    diff = Diff::new(left_ln, &fake_line);
                    dispatch_to_output(&mut output, &diff, &mut already_dispatched);
                    continue;
                };

                diff = Diff::new(left_ln, right_ln);
                dispatch_to_output(&mut output, &diff, &mut already_dispatched);
            }
            Result::Right(right_ln) => {
                let diff;
                let fake_line = Line::new(right_ln.line_ndx, &[]);
                let Some(left_ln) = left_lines.get(right_ln.line_ndx) else {
                    diff = Diff::new(&fake_line, right_ln);
                    dispatch_to_output(&mut output, &diff, &mut already_dispatched);
                    continue;
                };

                diff = Diff::new(left_ln, right_ln);
                dispatch_to_output(&mut output, &diff, &mut already_dispatched);
            }
            Result::Both(line1, line2) => {
                // Both are equal, complete diff
                dispatch_to_output(
                    &mut output,
                    &Diff::new(line1, line2),
                    &mut already_dispatched,
                );
            }
        }
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_both_files_empty() {
        let from = vec![];
        let to = vec![];
        assert_eq!(diff(&from, &to), vec![]);
    }

    #[test]
    fn test_left_empty_right_non_empty() {
        let from = vec![];
        let to = b"line1\nline2".to_vec();
        let mut expected = Vec::new();
        let eol: &[u8] = if cfg!(target_os = "windows") {
            b"\r\n"
        } else {
            b"\n"
        };

        expected.extend([b' '; 61 + 1]);
        expected.extend(b"> line1");
        expected.extend(eol);
        expected.extend([b' '; 61 + 1]);
        expected.extend(b"> line2");
        expected.extend(eol);

        assert_eq!(diff(&from, &to), expected);
    }

    #[test]
    fn test_right_empty_left_non_empty() {
        let from = b"line1\nline2".to_vec();
        let to = vec![];
        let mut expected = Vec::new();
        let eol: &[u8] = if cfg!(target_os = "windows") {
            b"\r\n"
        } else {
            b"\n"
        };

        expected.extend(b"line1");
        expected.extend([b' '; 61 - 5 + 1]);
        expected.extend(b"< ");
        expected.extend(eol);
        expected.extend(b"line2");
        expected.extend([b' '; 61 - 5 + 1]);
        expected.extend(b"< ");
        expected.extend(eol);

        assert_eq!(diff(&from, &to), expected);
    }

    #[test]
    fn test_identical_content() {
        let content = b"abc\n123".to_vec();
        let mut expected = Vec::new();
        let eol: &[u8] = if cfg!(target_os = "windows") {
            b"\r\n"
        } else {
            b"\n"
        };

        expected.extend(b"abc");
        expected.extend([b' '; 61 - 3 + 1]);
        expected.extend(b"  abc");
        expected.extend(eol);
        expected.extend(b"123");
        expected.extend([b' '; 61 - 3 + 1]);
        expected.extend(b"  123");
        expected.extend(eol);

        assert_eq!(diff(&content, &content), expected);
    }

    #[test]
    fn test_added_lines_in_right() {
        let from = b"a\nb".to_vec();
        let to = b"a\nb\nc".to_vec();
        let mut expected = Vec::new();
        let eol: &[u8] = if cfg!(target_os = "windows") {
            b"\r\n"
        } else {
            b"\n"
        };

        expected.extend(b"a");
        expected.extend([b' '; 61 - 1 + 1]);
        expected.extend(b"  a");
        expected.extend(eol);
        expected.extend(b"b");
        expected.extend([b' '; 61 - 1 + 1]);
        expected.extend(b"  b");
        expected.extend(eol);
        expected.extend([b' '; 62]);
        expected.extend(b"> c");
        expected.extend(eol);

        assert_eq!(diff(&from, &to), expected);
    }

    #[test]
    fn test_removed_lines_from_left() {
        let from = b"a\nb\nc".to_vec();
        let to = b"a\nb".to_vec();
        let mut expected = Vec::new();
        let eol: &[u8] = if cfg!(target_os = "windows") {
            b"\r\n"
        } else {
            b"\n"
        };

        expected.extend(b"a");
        expected.extend([b' '; 61 - 1 + 1]);
        expected.extend(b"  a");
        expected.extend(eol);
        expected.extend(b"b");
        expected.extend([b' '; 61 - 1 + 1]);
        expected.extend(b"  b");
        expected.extend(eol);
        expected.extend(b"c");
        expected.extend([b' '; 61 - 1 + 1]);
        expected.extend(b"< ");
        expected.extend(eol);

        assert_eq!(diff(&from, &to), expected);
    }

    #[test]
    fn test_modified_lines() {
        let from = b"original".to_vec();
        let to = b"modified".to_vec();
        let mut expected = Vec::new();
        let eol: &[u8] = if cfg!(target_os = "windows") {
            b"\r\n"
        } else {
            b"\n"
        };

        expected.extend(b"original");
        let left_len = 8;
        let spaces = 61 - left_len + 1;
        expected.extend(vec![b' '; spaces as usize]);
        expected.extend(b"| modified");
        expected.extend(eol);

        assert_eq!(diff(&from, &to), expected);
    }

    #[test]
    fn test_mixed_changes() {
        let from = b"a\nb\nc".to_vec();
        let to = b"a\nmodified\nnew".to_vec();
        let mut expected = Vec::new();
        let eol: &[u8] = if cfg!(target_os = "windows") {
            b"\r\n"
        } else {
            b"\n"
        };

        expected.extend(b"a");
        expected.extend([b' '; 61 - 1 + 1]);
        expected.extend(b"  a");
        expected.extend(eol);
        expected.extend(b"b");
        expected.extend([b' '; 61 - 1 + 1]);
        expected.extend(b"| modified");
        expected.extend(eol);
        expected.extend(b"c");
        expected.extend([b' '; 61 - 1 + 1]);
        expected.extend(b"| new");
        expected.extend(eol);

        assert_eq!(diff(&from, &to), expected);
    }

    #[test]
    fn test_no_duplicate_dispatch() {
        let from = b"a\na".to_vec();
        let to = b"a".to_vec();
        let output = diff(&from, &to);
        let expected_lines = if cfg!(target_os = "windows") { 4 } else { 2 };
        assert_eq!(
            output.iter().filter(|&&b| b == b'\n' || b == b'\r').count(),
            expected_lines
        );
    }
}
