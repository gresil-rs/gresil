// SPDX-FileCopyrightText: 2026 philo-the-dendron
// SPDX-License-Identifier: AGPL-3.0-or-later
//! gresil-trace-diff: a replay's capture against the host's trace files,
//! the board's judge (the frozen-format sitting's Call 7 (e), ruled
//! 2026-10-24). Pure std: no dependency, no `unsafe`. It ports
//! `tools/esp32c3_trace_diff.py`, the board's diff through round 78, and
//! prints what that script printed, but for the differences below.
//!
//! The firmware (`firmware/esp32c3/src/main.rs`) replays every frozen
//! case of `crates/gresil/tests/traces/frozen.rs`: each prints its
//! trace's header line, then its rows, and the replay ends with one line,
//! `# gresil-trace end`. The diff takes the frozen set from the files
//! themselves (every trace whose header says `plasticity=off`), splits
//! the log at the headers, drops carriage returns, and compares each case
//! with its file line for line: one line per case, exit 1 on any
//! difference. A header missing or repeated, a header outside the frozen
//! set, or no end line after the last case is a difference too: a capture
//! cut off is red, not a shorter compare. Any capture with the headers and
//! the end line reads the same way, the C3's or QEMU's
//! (`proofs/qemu-trace-replay/`, the same replay on riscv64gc).
//!
//! Through alpha.9 the format's name was `neuralos-trace`
//! (`crates/gresil/src/trace.rs`, the module doc), and what was written
//! then carries it: a line that opens with `# neuralos-trace ` compares as
//! if it opened with `# gresil-trace `, in a header line and in the end
//! line, so the captures banked under `evidence/` read as they did. A
//! difference prints the lines as the files hold them.
//!
//! `--trace FILE`, repeatable, adds a trace from outside the tree to the
//! set: a stranger's graph in the firmware's slot
//! (`firmware/esp32c3/stranger.sh`), replayed after the frozen cases,
//! whose trace `gresil-nir2json --freeze` wrote. Its header must say
//! `plasticity=off` and must not be in the set already; a file that does
//! not read as text is a usage error too. Two files with one header, in
//! the tree or given, are a usage error, never one case silently standing
//! for two. A given trace prints as `<stem> (given)`, so a stem that
//! matches a tree case never reads as a duplicate. The last line counts
//! every case of the set.
//!
//! Read as the Python read them: the log's bytes as UTF-8 with each bad
//! sequence replaced, a trace file as strict UTF-8 in Python's text mode
//! (`\r\n` and a lone `\r` each end a line), the tree's files every
//! `*.trace` of the directory in name order, a stem as Python 3.14's
//! `PurePath.stem`. A LOG or a tree trace that cannot be read is exit 1,
//! as the Python's uncaught error was. One difference on stdout, kept on
//! purpose: a line in a `DIFFERS` report is quoted as Python's `ascii()`
//! quotes it, where the Python used `repr()`. The two are the same on
//! ASCII, all the trace writer writes and all the banked captures hold;
//! `repr()` leaves the characters its Unicode tables call printable as
//! they are, and those tables move with the Python release. Two more,
//! which neither caller reaches (each builds the paths it passes, the
//! tree's names are ASCII, and `stranger.sh`'s converter stops on a graph
//! name that is not UTF-8 before the diff runs): a path that ends in a
//! slash does not read, where Python's `pathlib` read the file, and a file
//! name that is not UTF-8 prints with U+FFFD and sorts by its bytes.
//!
//! The command line, from the repository's root (`main.rs`):
//! `cargo run -q -p gresil-trace-diff -- [--trace FILE]... LOG`.
//! Exit: 0 every case identical · 1 a difference, or a LOG or a tree
//! trace that cannot be read · 2 usage.

#![forbid(unsafe_code)]

use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

/// A header line's opening, under the format's name.
const HEAD: &str = "# gresil-trace v1 case=";
/// The line that ends a replay.
pub const END: &str = "# gresil-trace end";
/// The name's opening through alpha.9, read as [`NEW`].
const OLD: &str = "# neuralos-trace ";
/// The name's opening since round 78.
const NEW: &str = "# gresil-trace ";
/// The usage line, printed after a usage error.
pub const USAGE: &str = "usage: gresil-trace-diff [--trace FILE]... LOG";

/// What a run prints and how it ends.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Report {
    /// The report: one line per header outside the set, one per case,
    /// the cut-off line if the capture is cut off, then the count.
    pub stdout: String,
    /// A usage error and the usage line, or why a file could not be read.
    pub stderr: String,
    /// 0 every case identical · 1 a difference, or a LOG or a tree trace
    /// that cannot be read · 2 usage.
    pub code: u8,
}

/// Why a run stopped before the compare.
enum Stop {
    /// A usage error: the reason, then the usage line, exit 2.
    Usage(String),
    /// A file the run needs could not be read: the reason, exit 1.
    Fail(String),
}

/// One case of the set: its header under the format's name, the name it
/// prints as, and its file's lines, the header first.
struct Case {
    header: String,
    name: String,
    lines: Vec<String>,
}

/// The diff of the capture `args` names against the plasticity-off traces
/// of `traces` and the `--trace` files, `args` being the command line
/// after the program's name.
pub fn run(args: &[OsString], traces: &Path) -> Report {
    let mut report = Report::default();
    match diff(args, traces, &mut report.stdout) {
        Ok(red) => report.code = u8::from(red > 0),
        Err(Stop::Usage(why)) => {
            report.stderr = format!("gresil-trace-diff: {why}\n{USAGE}\n");
            report.code = 2;
        }
        Err(Stop::Fail(why)) => {
            report.stderr = format!("gresil-trace-diff: {why}\n");
            report.code = 1;
        }
    }
    report
}

/// The run, in the Python's order: the arguments, the log, the tree's
/// traces, the given ones, then the compare into `out`; the count of red
/// lines.
fn diff(args: &[OsString], traces: &Path, out: &mut String) -> Result<usize, Stop> {
    let mut given = Vec::new();
    let mut logs = Vec::new();
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        if arg == "--trace" {
            let Some(file) = args.next() else {
                return Err(Stop::Usage("--trace needs FILE".into()));
            };
            given.push(PathBuf::from(file));
        } else if arg.as_encoded_bytes().starts_with(b"--") {
            return Err(Stop::Usage(format!(
                "unknown flag {}",
                arg.to_string_lossy()
            )));
        } else {
            logs.push(Path::new(arg));
        }
    }
    let [log] = logs[..] else {
        return Err(Stop::Usage("one LOG".into()));
    };
    let bytes = fs::read(log).map_err(|e| Stop::Fail(format!("{}: {e}", log.display())))?;
    let log = lines_of(&String::from_utf8_lossy(&bytes));

    let mut cases: Vec<Case> = Vec::new();
    for path in tree_traces(traces) {
        let text = read_text(&path).map_err(|e| Stop::Fail(format!("{}: {e}", path.display())))?;
        let lines = lines_of(&text);
        if let Some(first) = lines.first()
            && first.contains(" plasticity=off ")
        {
            let header = canon(first);
            if let Some(case) = cases.iter().find(|c| c.header == header) {
                let why = format!(
                    "{} and {} share one header: {first}",
                    stem(&path),
                    case.name
                );
                return Err(Stop::Usage(why));
            }
            cases.push(Case {
                header,
                name: stem(&path),
                lines,
            });
        }
    }
    if cases.is_empty() {
        return Err(Stop::Fail(format!(
            "no plasticity-off trace in {}",
            traces.display()
        )));
    }
    for path in &given {
        let text =
            read_text(path).map_err(|e| Stop::Usage(format!("--trace {}: {e}", path.display())))?;
        let lines = lines_of(&text);
        let Some(first) = lines
            .first()
            .filter(|l| canon(l).starts_with(HEAD) && l.contains(" plasticity=off "))
        else {
            return Err(Stop::Usage(format!(
                "--trace {}: not a plasticity-off trace header",
                path.display()
            )));
        };
        let header = canon(first);
        if let Some(case) = cases.iter().find(|c| c.header == header) {
            return Err(Stop::Usage(format!(
                "--trace {}: its header is already in the set ({})",
                path.display(),
                case.name
            )));
        }
        cases.push(Case {
            header,
            name: format!("{} (given)", stem(path)),
            lines,
        });
    }
    Ok(compare(&log, &cases, out))
}

/// Each case against its stretch of the log, one line each into `out`,
/// then the cut-off line and the count; the count of red lines.
fn compare(log: &[String], cases: &[Case], out: &mut String) -> usize {
    let heads: Vec<usize> = (0..log.len())
        .filter(|&i| canon(&log[i]).starts_with(HEAD))
        .collect();
    let ends: Vec<usize> = (0..log.len()).filter(|&i| canon(&log[i]) == END).collect();
    let mut red = 0;
    for &i in &heads {
        if !cases.iter().any(|c| c.header == canon(&log[i])) {
            out.push_str(&format!(
                "log line {}: a header outside the frozen set: {}\n",
                i + 1,
                log[i]
            ));
            red += 1;
        }
    }
    for case in cases {
        let at: Vec<usize> = heads
            .iter()
            .copied()
            .filter(|&i| canon(&log[i]) == case.header)
            .collect();
        let [start] = at[..] else {
            let place = if at.is_empty() {
                "missing from the log".to_string()
            } else {
                format!("{} times in the log", at.len())
            };
            out.push_str(&format!("{}: {place}\n", case.name));
            red += 1;
            continue;
        };
        let stop = heads
            .iter()
            .chain(&ends)
            .copied()
            .filter(|&i| i > start)
            .min()
            .unwrap_or(log.len());
        let (want, have) = (&case.lines, &log[start..stop]);
        if want.len() == have.len() && want.iter().zip(have).all(|(a, b)| canon(a) == canon(b)) {
            out.push_str(&format!(
                "{}: identical, {} rows\n",
                case.name,
                want.len() - 1
            ));
            continue;
        }
        let n = want
            .iter()
            .zip(have)
            .position(|(a, b)| canon(a) != canon(b))
            .unwrap_or(want.len().min(have.len()));
        let file = want.get(n).map_or("<end>", String::as_str);
        let got = have.get(n).map_or("<end>", String::as_str);
        out.push_str(&format!(
            "{}: DIFFERS at line {}: file {}, log {}\n",
            case.name,
            n + 1,
            quote(file),
            quote(got)
        ));
        red += 1;
    }
    let cut = match (ends.last(), heads.last()) {
        (None, _) => true,
        (Some(end), Some(head)) => end < head,
        (Some(_), None) => false,
    };
    if cut {
        out.push_str(&format!(
            "no {} line after the last case: the capture is cut off\n",
            quote(END)
        ));
        red += 1;
    }
    out.push_str(&format!("{} cases, {red} red\n", cases.len()));
    red
}

/// Every `*.trace` entry of `dir`, hidden ones too, in name order, as the
/// Python's sorted glob gave them; none when `dir` cannot be read.
fn tree_traces(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut names: Vec<OsString> = entries
        .filter_map(Result::ok)
        .map(|e| e.file_name())
        .filter(|n| n.as_encoded_bytes().ends_with(b".trace"))
        .collect();
    names.sort();
    names.into_iter().map(|n| dir.join(n)).collect()
}

/// A trace file's text as Python's text mode reads it: strict UTF-8, and
/// `\r\n` and a lone `\r` each a line end.
fn read_text(path: &Path) -> Result<String, String> {
    let bytes = fs::read(path).map_err(|e| e.to_string())?;
    let text = String::from_utf8(bytes).map_err(|e| e.to_string())?;
    Ok(text.replace("\r\n", "\n").replace('\r', "\n"))
}

/// The lines of a text, carriage returns dropped, no trailing empty line.
fn lines_of(text: &str) -> Vec<String> {
    let mut lines: Vec<String> = text
        .replace('\r', "")
        .split('\n')
        .map(String::from)
        .collect();
    if lines.last().is_some_and(String::is_empty) {
        lines.pop();
    }
    lines
}

/// A line under the format's name: [`OLD`] read as [`NEW`].
fn canon(line: &str) -> String {
    match line.strip_prefix(OLD) {
        Some(rest) => format!("{NEW}{rest}"),
        None => line.to_string(),
    }
}

/// A file's name without its last suffix, as Python 3.14's
/// `PurePath.stem` gives it: the dots a name opens with are never a
/// suffix's.
fn stem(path: &Path) -> String {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let dots = name.len() - name.trim_start_matches('.').len();
    match name[dots..].rfind('.') {
        Some(i) => name[..dots + i].to_string(),
        None => name,
    }
}

/// A line quoted as Python's `ascii()` quotes a string: in single quotes,
/// or double ones when it holds a single quote and no double one; a
/// backslash and that quote behind a backslash, `\t`, `\n` and `\r` by
/// name, and every other character outside printable ASCII as `\xhh`,
/// `\uhhhh` or `\Uhhhhhhhh`.
fn quote(line: &str) -> String {
    let q = if line.contains('\'') && !line.contains('"') {
        '"'
    } else {
        '\''
    };
    let mut out = String::from(q);
    for c in line.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '\t' => out.push_str("\\t"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            ' '..='~' if c == q => {
                out.push('\\');
                out.push(c);
            }
            ' '..='~' => out.push(c),
            _ if u32::from(c) < 0x100 => out.push_str(&format!("\\x{:02x}", u32::from(c))),
            _ if u32::from(c) < 0x10000 => out.push_str(&format!("\\u{:04x}", u32::from(c))),
            _ => out.push_str(&format!("\\U{:08x}", u32::from(c))),
        }
    }
    out.push(q);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quote_takes_python_ascii_quotes() {
        assert_eq!(quote("plain"), "'plain'");
        assert_eq!(quote("it's"), "\"it's\"");
        assert_eq!(quote("say \"x\""), "'say \"x\"'");
        assert_eq!(quote("both ' and \""), "'both \\' and \"'");
        assert_eq!(quote("<end>"), "'<end>'");
    }

    #[test]
    fn quote_escapes_as_python_ascii() {
        assert_eq!(quote("back\\slash"), "'back\\\\slash'");
        assert_eq!(quote("tab\there"), "'tab\\there'");
        assert_eq!(quote("a\nb\rc"), "'a\\nb\\rc'");
        assert_eq!(quote("\u{0}\u{1f}\u{7f} ~"), "'\\x00\\x1f\\x7f ~'");
        assert_eq!(
            quote("\u{a0}\u{ad}\u{85}\u{e9}\u{ff}"),
            "'\\xa0\\xad\\x85\\xe9\\xff'"
        );
        assert_eq!(quote("\u{100}\u{fffd}\u{ffff}"), "'\\u0100\\ufffd\\uffff'");
        assert_eq!(
            quote("\u{10000}\u{1f600}\u{10ffff}"),
            "'\\U00010000\\U0001f600\\U0010ffff'"
        );
    }

    #[test]
    fn stem_is_python_314s() {
        for (path, want) in [
            ("a/foo.trace", "foo"),
            ("foo.trace", "foo"),
            ("a.b.trace", "a.b"),
            (".trace", ".trace"),
            ("..trace", "..trace"),
            ("x/.y.trace", ".y"),
            ("foo.", "foo"),
            ("foo", "foo"),
            ("foo..trace", "foo."),
        ] {
            assert_eq!(stem(Path::new(path)), want, "{path}");
        }
    }

    #[test]
    fn lines_of_drops_carriage_returns_and_one_trailing_empty_line() {
        assert_eq!(lines_of("a\r\nb\rc\n"), ["a", "bc"]);
        assert_eq!(lines_of("a\n\n"), ["a", ""]);
        assert_eq!(lines_of(""), Vec::<String>::new());
        assert_eq!(lines_of("a"), ["a"]);
    }

    #[test]
    fn canon_reads_the_old_name_at_a_line_start_only() {
        assert_eq!(
            canon("# neuralos-trace v1 case=a"),
            "# gresil-trace v1 case=a"
        );
        assert_eq!(canon("# neuralos-trace end"), END);
        assert_eq!(canon(" # neuralos-trace end"), " # neuralos-trace end");
        assert_eq!(canon("# neuralos-traceend"), "# neuralos-traceend");
        assert_eq!(canon("# gresil-trace end"), END);
    }
}
