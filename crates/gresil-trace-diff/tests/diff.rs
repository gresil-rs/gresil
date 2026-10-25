// SPDX-FileCopyrightText: 2026 philo-the-dendron
// SPDX-License-Identifier: AGPL-3.0-or-later
//! The diff's promises (the library's module doc) as tests, each on a
//! directory of traces made here: the replay read case by case, the
//! failure cases, the old name, the `--trace` rules, the usage errors;
//! then the binary on the tree's own traces, and its refusal to judge for
//! another place than the one it was built in.

use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use gresil_trace_diff::{END, Report, USAGE, run};

/// A header line of case `case` under the name `name` ("gresil" or
/// "neuralos"), plasticity `plasticity`.
fn header(name: &str, case: &str, plasticity: &str) -> String {
    format!(
        "# {name}-trace v1 case={case} kind=regression n=1 dt_us=1000 res=cmV plasticity={plasticity} divisor=1 steps=3 rows=all"
    )
}

/// Case `case`'s trace under the new name, plasticity off: its header,
/// then `rows` rows of its own.
fn trace(case: &str, rows: usize) -> Vec<String> {
    let mut lines = vec![header("gresil", case, "off")];
    lines.extend((0..rows).map(|i| format!("{i} {} | 0 | -7000 {case}", i * 100)));
    lines
}

/// The same trace with its header under `name`.
fn named(name: &str, mut lines: Vec<String>) -> Vec<String> {
    lines[0] = lines[0].replacen("# gresil-trace ", &format!("# {name}-trace "), 1);
    lines
}

/// A scratch directory under the target dir, empty.
fn scratch(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("traces")).expect("the scratch directory");
    dir
}

/// Writes `lines` with a `\n` after each, as the writer does.
fn write(path: &Path, lines: &[String]) {
    fs::write(
        path,
        lines.iter().map(|l| format!("{l}\n")).collect::<String>(),
    )
    .expect("a file");
}

/// The scratch directory `name` with its `traces/`: `alpha` (2 rows),
/// `beta` (3 rows), `learning` (plasticity on), and `frozen.rs`.
fn tree(name: &str) -> PathBuf {
    let dir = scratch(name);
    write(&dir.join("traces/alpha.trace"), &trace("alpha", 2));
    write(&dir.join("traces/beta.trace"), &trace("beta", 3));
    let mut learning = trace("learning", 1);
    learning[0] = header("gresil", "learning", "on");
    write(&dir.join("traces/learning.trace"), &learning);
    fs::write(dir.join("traces/frozen.rs"), "// the frozen file\n").expect("frozen.rs");
    dir
}

/// A capture: the fragment before the reset and the boot lines, each
/// part's lines, then `end` if given, then the loop's lines after it.
fn capture(parts: &[Vec<String>], end: Option<&str>) -> String {
    let mut text = String::from(
        "spike at 23732090 us (step 23651)\nESP-ROM:esp32c3-api1-20210207\r\nentry 0x403cbf1a\r\n",
    );
    for line in parts.iter().flatten() {
        text.push_str(line);
        text.push('\n');
    }
    if let Some(end) = end {
        text.push_str(end);
        text.push('\n');
    }
    text.push_str(
        "stack: 1368 of 316316 bytes high-water after the replays\nspike at 55156 us (step 55)\n",
    );
    text
}

/// The diff of the log `text`, written beside `dir`'s traces, with
/// `--trace` for each of `given`.
fn diff(dir: &Path, text: &str, given: &[&Path]) -> Report {
    let log = dir.join("capture.log");
    fs::write(&log, text).expect("the capture");
    let mut args: Vec<OsString> = Vec::new();
    for g in given {
        args.push("--trace".into());
        args.push(g.into());
    }
    args.push(log.into());
    run(&args, &dir.join("traces"))
}

/// A report of `stdout` and exit `code`, nothing on stderr.
fn report(stdout: &[&str], code: u8) -> Report {
    Report {
        stdout: stdout.iter().map(|l| format!("{l}\n")).collect(),
        stderr: String::new(),
        code,
    }
}

/// The usage error `why`: nothing on stdout, exit 2.
fn usage(why: &str) -> Report {
    Report {
        stdout: String::new(),
        stderr: format!("gresil-trace-diff: {why}\n{USAGE}\n"),
        code: 2,
    }
}

const GREEN: [&str; 3] = [
    "alpha: identical, 2 rows",
    "beta: identical, 3 rows",
    "2 cases, 0 red",
];

#[test]
fn a_replay_reads_green_case_by_case() {
    let dir = tree("green");
    let r = diff(
        &dir,
        &capture(&[trace("alpha", 2), trace("beta", 3)], Some(END)),
        &[],
    );
    assert_eq!(r, report(&GREEN, 0));
}

#[test]
fn a_header_on_the_log_s_first_line_is_read() {
    let dir = tree("first-line-header");
    let mut text = String::new();
    for line in trace("alpha", 2).iter().chain(&trace("beta", 3)) {
        text.push_str(line);
        text.push('\n');
    }
    text.push_str(END);
    text.push('\n');
    assert_eq!(diff(&dir, &text, &[]), report(&GREEN, 0));
}

#[test]
fn a_changed_row_differs_at_its_line() {
    let dir = tree("changed-row");
    let mut beta = trace("beta", 3);
    beta[2] = "1 101 | 0 | -7000 beta".into();
    let r = diff(&dir, &capture(&[trace("alpha", 2), beta], Some(END)), &[]);
    let want = [
        "alpha: identical, 2 rows",
        "beta: DIFFERS at line 3: file '1 100 | 0 | -7000 beta', log '1 101 | 0 | -7000 beta'",
        "2 cases, 1 red",
    ];
    assert_eq!(r, report(&want, 1));
}

#[test]
fn a_capture_cut_off_mid_case_is_red() {
    let dir = tree("cut-mid-case");
    let mut beta = trace("beta", 3);
    beta.truncate(3);
    let text = capture(&[trace("alpha", 2), beta], None);
    let text = text.split("stack:").next().unwrap();
    let r = diff(&dir, text, &[]);
    let want = [
        "alpha: identical, 2 rows",
        "beta: DIFFERS at line 4: file '2 200 | 0 | -7000 beta', log '<end>'",
        "no '# gresil-trace end' line after the last case: the capture is cut off",
        "2 cases, 2 red",
    ];
    assert_eq!(r, report(&want, 1));
}

#[test]
fn a_line_before_the_next_header_makes_a_case_longer() {
    let dir = tree("longer");
    let mut alpha = trace("alpha", 2);
    alpha.push("spike at 1000 us (step 1)".into());
    let r = diff(&dir, &capture(&[alpha, trace("beta", 3)], Some(END)), &[]);
    let want = [
        "alpha: DIFFERS at line 4: file '<end>', log 'spike at 1000 us (step 1)'",
        "beta: identical, 3 rows",
        "2 cases, 1 red",
    ];
    assert_eq!(r, report(&want, 1));
}

#[test]
fn a_missing_header_is_red() {
    let dir = tree("missing");
    let r = diff(&dir, &capture(&[trace("alpha", 2)], Some(END)), &[]);
    assert_eq!(
        r,
        report(
            &[
                "alpha: identical, 2 rows",
                "beta: missing from the log",
                "2 cases, 1 red"
            ],
            1
        )
    );
}

#[test]
fn a_repeated_header_is_red() {
    let dir = tree("repeated");
    let r = diff(
        &dir,
        &capture(
            &[trace("alpha", 2), trace("alpha", 2), trace("beta", 3)],
            Some(END),
        ),
        &[],
    );
    let want = [
        "alpha: 2 times in the log",
        "beta: identical, 3 rows",
        "2 cases, 1 red",
    ];
    assert_eq!(r, report(&want, 1));
}

#[test]
fn a_header_outside_the_set_is_red_and_printed_as_the_log_holds_it() {
    let dir = tree("outside");
    let mut gamma = trace("gamma", 1);
    gamma[0] = gamma[0].replacen("case=gamma", "case=gamma\u{fffd}", 1);
    let text = capture(
        &[trace("alpha", 2), trace("beta", 3), gamma.clone()],
        Some(END),
    );
    let text = text.replacen("case=gamma\u{fffd}", "case=gamma\u{ff}", 1);
    let mut bytes = text.into_bytes();
    let at = bytes
        .windows(2)
        .position(|w| w == "\u{ff}".as_bytes())
        .unwrap();
    bytes.splice(at..at + 2, [0xff]);
    let log = dir.join("capture.log");
    fs::write(&log, bytes).unwrap();
    let r = run(&[log.into()], &dir.join("traces"));
    let want = [
        format!(
            "log line {}: a header outside the frozen set: {}",
            3 + 3 + 4 + 1,
            gamma[0]
        ),
        "alpha: identical, 2 rows".into(),
        "beta: identical, 3 rows".into(),
        "2 cases, 1 red".into(),
    ];
    assert_eq!(
        r,
        report(&want.iter().map(String::as_str).collect::<Vec<_>>(), 1)
    );
}

#[test]
fn an_old_name_header_outside_the_set_prints_with_its_old_name() {
    let dir = tree("outside-old-name");
    let omega = named("neuralos", trace("omega", 1));
    let r = diff(
        &dir,
        &capture(
            &[trace("alpha", 2), trace("beta", 3), omega.clone()],
            Some(END),
        ),
        &[],
    );
    let want = [
        format!("log line 11: a header outside the frozen set: {}", omega[0]),
        "alpha: identical, 2 rows".into(),
        "beta: identical, 3 rows".into(),
        "2 cases, 1 red".into(),
    ];
    assert_eq!(
        r,
        report(&want.iter().map(String::as_str).collect::<Vec<_>>(), 1)
    );
}

#[test]
fn an_end_line_and_no_header_is_not_a_capture_cut_off() {
    let dir = tree("end-only");
    let r = diff(&dir, &capture(&[], Some(END)), &[]);
    let want = [
        "alpha: missing from the log",
        "beta: missing from the log",
        "2 cases, 2 red",
    ];
    assert_eq!(r, report(&want, 1));
}

#[test]
fn the_tree_s_traces_read_in_name_order() {
    let dir = scratch("name-order");
    let names = ["h", "g", "f", "e", "d", "c", "b", "a"];
    for name in names {
        write(
            &dir.join("traces").join(format!("{name}.trace")),
            &trace(name, 1),
        );
    }
    let parts: Vec<Vec<String>> = names.iter().map(|n| trace(n, 1)).collect();
    let r = diff(&dir, &capture(&parts, Some(END)), &[]);
    let mut want: Vec<String> = names
        .iter()
        .rev()
        .map(|n| format!("{n}: identical, 1 rows"))
        .collect();
    want.push("8 cases, 0 red".into());
    assert_eq!(
        r,
        report(&want.iter().map(String::as_str).collect::<Vec<_>>(), 0)
    );
}

#[test]
fn a_plasticity_on_trace_is_not_in_the_set() {
    let dir = tree("plasticity-on");
    let mut learning = trace("learning", 1);
    learning[0] = header("gresil", "learning", "on");
    let r = diff(
        &dir,
        &capture(
            &[trace("alpha", 2), trace("beta", 3), learning.clone()],
            Some(END),
        ),
        &[],
    );
    let want = [
        format!(
            "log line 11: a header outside the frozen set: {}",
            learning[0]
        ),
        "alpha: identical, 2 rows".into(),
        "beta: identical, 3 rows".into(),
        "2 cases, 1 red".into(),
    ];
    assert_eq!(
        r,
        report(&want.iter().map(String::as_str).collect::<Vec<_>>(), 1)
    );
}

#[test]
fn no_end_line_is_a_capture_cut_off() {
    let dir = tree("no-end");
    let r = diff(
        &dir,
        &capture(&[trace("alpha", 2), trace("beta", 3)], None),
        &[],
    );
    let want = [
        "alpha: identical, 2 rows",
        "beta: DIFFERS at line 5: file '<end>', log 'stack: 1368 of 316316 bytes high-water after the replays'",
        "no '# gresil-trace end' line after the last case: the capture is cut off",
        "2 cases, 2 red",
    ];
    assert_eq!(r, report(&want, 1));
}

#[test]
fn an_end_line_before_the_last_header_is_a_capture_cut_off() {
    let dir = tree("reboot");
    let mut text = capture(&[trace("alpha", 2), trace("beta", 3)], Some(END));
    text.push_str(&capture(&[trace("alpha", 2)], None));
    let r = diff(&dir, &text, &[]);
    let want = [
        "alpha: 2 times in the log",
        "beta: identical, 3 rows",
        "no '# gresil-trace end' line after the last case: the capture is cut off",
        "2 cases, 2 red",
    ];
    assert_eq!(r, report(&want, 1));
}

#[test]
fn the_log_s_carriage_returns_are_dropped() {
    let dir = tree("crlf-log");
    let text = capture(&[trace("alpha", 2), trace("beta", 3)], Some(END)).replace('\n', "\r\n");
    let text = text.replacen("0 0 | 0", "0 0\r | 0", 1);
    assert_eq!(diff(&dir, &text, &[]), report(&GREEN, 0));
}

#[test]
fn a_trace_file_reads_in_python_s_text_mode() {
    let dir = tree("text-mode");
    let alpha: String = trace("alpha", 2)
        .iter()
        .map(|l| format!("{l}\r\n"))
        .collect();
    fs::write(dir.join("traces/alpha.trace"), alpha).unwrap();
    let beta: Vec<String> = trace("beta", 3);
    let joined = format!("{}\n{}\r{}\n{}\n", beta[0], beta[1], beta[2], beta[3]);
    fs::write(dir.join("traces/beta.trace"), joined).unwrap();
    let r = diff(
        &dir,
        &capture(&[trace("alpha", 2), trace("beta", 3)], Some(END)),
        &[],
    );
    assert_eq!(r, report(&GREEN, 0));
}

#[test]
fn only_trace_files_are_read_hidden_ones_too() {
    let dir = tree("only-traces");
    for decoy in [
        "frozen.rs",
        "delta.trace.bak",
        "delta.TRACE",
        "delta.notatrace",
    ] {
        write(&dir.join("traces").join(decoy), &trace("delta", 1));
    }
    write(&dir.join("traces/.hidden.trace"), &trace("hidden", 1));
    let r = diff(
        &dir,
        &capture(
            &[trace("hidden", 1), trace("alpha", 2), trace("beta", 3)],
            Some(END),
        ),
        &[],
    );
    let want = [
        ".hidden: identical, 1 rows",
        "alpha: identical, 2 rows",
        "beta: identical, 3 rows",
        "3 cases, 0 red",
    ];
    assert_eq!(r, report(&want, 0));
}

#[test]
fn no_plasticity_off_trace_is_exit_1() {
    let dir = tree("none-off");
    fs::remove_file(dir.join("traces/alpha.trace")).unwrap();
    fs::remove_file(dir.join("traces/beta.trace")).unwrap();
    let r = diff(&dir, &capture(&[], Some(END)), &[]);
    let why = format!(
        "gresil-trace-diff: no plasticity-off trace in {}\n",
        dir.join("traces").display()
    );
    assert_eq!(
        r,
        Report {
            stdout: String::new(),
            stderr: why,
            code: 1
        }
    );
    let r = run(&[dir.join("capture.log").into()], &dir.join("no-such-dir"));
    assert_eq!((r.stdout.as_str(), r.code), ("", 1));
    assert!(
        r.stderr.contains("no plasticity-off trace in"),
        "{}",
        r.stderr
    );
}

#[test]
fn two_tree_files_with_one_header_are_a_usage_error() {
    let dir = tree("tree-twice");
    write(&dir.join("traces/copy.trace"), &trace("alpha", 1));
    let r = diff(&dir, &capture(&[], Some(END)), &[]);
    assert_eq!(
        r,
        usage(&format!(
            "copy and alpha share one header: {}",
            trace("alpha", 0)[0]
        ))
    );
    let dir = tree("tree-twice-named");
    let old = named("neuralos", trace("alpha", 1));
    write(&dir.join("traces/old.trace"), &old);
    let r = diff(&dir, &capture(&[], Some(END)), &[]);
    assert_eq!(
        r,
        usage(&format!("old and alpha share one header: {}", old[0]))
    );
}

#[test]
fn a_tree_trace_that_does_not_read_is_exit_1() {
    let dir = tree("tree-unreadable");
    fs::write(dir.join("traces/bad.trace"), b"# gresil-trace v1 \xff\n").unwrap();
    let r = diff(&dir, &capture(&[], Some(END)), &[]);
    assert_eq!((r.stdout.as_str(), r.code), ("", 1));
    assert!(r.stderr.starts_with(&format!(
        "gresil-trace-diff: {}: ",
        dir.join("traces/bad.trace").display()
    )));
    let dir = tree("tree-directory");
    fs::create_dir(dir.join("traces/dir.trace")).unwrap();
    let r = diff(&dir, &capture(&[], Some(END)), &[]);
    assert_eq!((r.stdout.as_str(), r.code), ("", 1));
}

#[test]
fn a_given_trace_joins_the_set_as_given() {
    let dir = tree("given");
    let graph = dir.join("graph.trace");
    write(&graph, &trace("stranger", 2));
    let r = diff(
        &dir,
        &capture(
            &[trace("alpha", 2), trace("beta", 3), trace("stranger", 2)],
            Some(END),
        ),
        &[&graph],
    );
    let want = [
        "alpha: identical, 2 rows",
        "beta: identical, 3 rows",
        "graph (given): identical, 2 rows",
        "3 cases, 0 red",
    ];
    assert_eq!(r, report(&want, 0));
    let r = diff(
        &dir,
        &capture(&[trace("alpha", 2), trace("beta", 3)], Some(END)),
        &[&graph],
    );
    let want = [
        "alpha: identical, 2 rows",
        "beta: identical, 3 rows",
        "graph (given): missing from the log",
        "3 cases, 1 red",
    ];
    assert_eq!(r, report(&want, 1));
}

#[test]
fn a_given_trace_must_open_with_a_plasticity_off_header() {
    let dir = tree("given-header");
    let on = dir.join("on.trace");
    let mut lines = trace("stranger", 1);
    lines[0] = header("gresil", "stranger", "on");
    write(&on, &lines);
    let other = dir.join("other.trace");
    write(&other, &[header("other", "stranger", "off"), "0 0".into()]);
    let empty = dir.join("empty.trace");
    fs::write(&empty, "").unwrap();
    for path in [&on, &other, &empty] {
        let r = diff(&dir, &capture(&[], Some(END)), &[path]);
        assert_eq!(
            r,
            usage(&format!(
                "--trace {}: not a plasticity-off trace header",
                path.display()
            ))
        );
    }
}

#[test]
fn a_given_trace_that_does_not_read_as_text_is_a_usage_error() {
    let dir = tree("given-unreadable");
    let bad = dir.join("bad.trace");
    fs::write(&bad, b"# gresil-trace v1 case=x plasticity=off \xff\n").unwrap();
    for path in [bad, dir.join("absent.trace"), dir.join("traces")] {
        let r = diff(&dir, &capture(&[], Some(END)), &[&path]);
        assert_eq!((r.stdout.as_str(), r.code), ("", 2), "{}", path.display());
        let head = format!("gresil-trace-diff: --trace {}: ", path.display());
        assert!(
            r.stderr.starts_with(&head) && r.stderr.ends_with(&format!("\n{USAGE}\n")),
            "{}",
            r.stderr
        );
    }
}

#[test]
fn a_given_trace_already_in_the_set_is_a_usage_error() {
    let dir = tree("given-twice");
    let copy = dir.join("copy.trace");
    write(&copy, &trace("alpha", 2));
    let r = diff(&dir, &capture(&[], Some(END)), &[&copy]);
    assert_eq!(
        r,
        usage(&format!(
            "--trace {}: its header is already in the set (alpha)",
            copy.display()
        ))
    );
    let one = dir.join("one.trace");
    write(&one, &trace("stranger", 1));
    let two = dir.join("two.trace");
    write(&two, &trace("stranger", 2));
    let r = diff(&dir, &capture(&[], Some(END)), &[&one, &two]);
    assert_eq!(
        r,
        usage(&format!(
            "--trace {}: its header is already in the set (one (given))",
            two.display()
        ))
    );
}

#[test]
fn a_given_duplicate_under_the_other_name_is_a_usage_error() {
    let dir = tree("given-other-name");
    let old = dir.join("old.trace");
    write(&old, &named("neuralos", trace("alpha", 2)));
    let r = diff(&dir, &capture(&[], Some(END)), &[&old]);
    assert_eq!(
        r,
        usage(&format!(
            "--trace {}: its header is already in the set (alpha)",
            old.display()
        ))
    );
}

#[test]
fn a_given_stem_that_matches_a_tree_case_is_not_a_duplicate() {
    let dir = tree("given-stem");
    fs::create_dir(dir.join("mine")).unwrap();
    let alpha = dir.join("mine/alpha.trace");
    write(&alpha, &trace("stranger", 1));
    let r = diff(
        &dir,
        &capture(
            &[trace("alpha", 2), trace("beta", 3), trace("stranger", 1)],
            Some(END),
        ),
        &[&alpha],
    );
    let want = [
        "alpha: identical, 2 rows",
        "beta: identical, 3 rows",
        "alpha (given): identical, 1 rows",
        "3 cases, 0 red",
    ];
    assert_eq!(r, report(&want, 0));
}

#[test]
fn an_old_name_capture_reads_as_it_did() {
    let dir = tree("old-capture");
    let parts = [
        named("neuralos", trace("alpha", 2)),
        named("neuralos", trace("beta", 3)),
    ];
    assert_eq!(
        diff(&dir, &capture(&parts, Some("# neuralos-trace end")), &[]),
        report(&GREEN, 0)
    );
}

#[test]
fn an_old_name_capture_with_a_changed_row_differs_at_the_row() {
    let dir = tree("old-changed-row");
    let mut beta = named("neuralos", trace("beta", 3));
    beta[2] = "1 101 | 0 | -7000 beta".into();
    let parts = [named("neuralos", trace("alpha", 2)), beta];
    let r = diff(&dir, &capture(&parts, Some("# neuralos-trace end")), &[]);
    let want = [
        "alpha: identical, 2 rows",
        "beta: DIFFERS at line 3: file '1 100 | 0 | -7000 beta', log '1 101 | 0 | -7000 beta'",
        "2 cases, 1 red",
    ];
    assert_eq!(r, report(&want, 1));
}

#[test]
fn old_headers_with_the_new_end_line_read_green() {
    let dir = tree("old-headers-new-end");
    let parts = [
        named("neuralos", trace("alpha", 2)),
        named("neuralos", trace("beta", 3)),
    ];
    assert_eq!(
        diff(&dir, &capture(&parts, Some(END)), &[]),
        report(&GREEN, 0)
    );
}

#[test]
fn old_name_traces_read_a_new_name_capture() {
    let dir = tree("old-traces");
    write(
        &dir.join("traces/alpha.trace"),
        &named("neuralos", trace("alpha", 2)),
    );
    let r = diff(
        &dir,
        &capture(&[trace("alpha", 2), trace("beta", 3)], Some(END)),
        &[],
    );
    assert_eq!(r, report(&GREEN, 0));
}

#[test]
fn one_case_under_both_names_in_one_capture_is_red() {
    let dir = tree("both-names");
    let parts = [
        trace("alpha", 2),
        named("neuralos", trace("alpha", 2)),
        trace("beta", 3),
    ];
    let r = diff(&dir, &capture(&parts, Some(END)), &[]);
    let want = [
        "alpha: 2 times in the log",
        "beta: identical, 3 rows",
        "2 cases, 1 red",
    ];
    assert_eq!(r, report(&want, 1));
}

#[test]
fn a_difference_prints_the_lines_as_the_files_hold_them() {
    let dir = tree("as-held");
    let mut alpha = trace("alpha", 2);
    alpha[1] = "# neuralos-trace note".into();
    let r = diff(&dir, &capture(&[alpha, trace("beta", 3)], Some(END)), &[]);
    let want = [
        "alpha: DIFFERS at line 2: file '0 0 | 0 | -7000 alpha', log '# neuralos-trace note'",
        "beta: identical, 3 rows",
        "2 cases, 1 red",
    ];
    assert_eq!(r, report(&want, 1));
}

#[test]
fn a_differs_line_quotes_as_python_s_ascii() {
    let dir = tree("quoted");
    let mut text = capture(&[trace("alpha", 2), trace("beta", 3)], Some(END)).into_bytes();
    let row = b"0 0 | 0 | -7000 beta";
    let at = text.windows(row.len()).position(|w| w == row).unwrap();
    text.splice(at..at + row.len(), b"0 beta's\t\\\xff".iter().copied());
    let log = dir.join("capture.log");
    fs::write(&log, text).unwrap();
    let r = run(&[log.into()], &dir.join("traces"));
    let want = [
        "alpha: identical, 2 rows",
        "beta: DIFFERS at line 2: file '0 0 | 0 | -7000 beta', log \"0 beta's\\t\\\\\\ufffd\"",
        "2 cases, 1 red",
    ];
    assert_eq!(r, report(&want, 1));
}

#[test]
fn the_usage_errors_are_exit_2() {
    let dir = tree("usage");
    let log = dir.join("capture.log");
    fs::write(&log, capture(&[], Some(END))).unwrap();
    let l = || OsString::from(&log);
    for (args, why) in [
        (vec![], "one LOG"),
        (vec![l(), l()], "one LOG"),
        (vec![l(), "--trace".into()], "--trace needs FILE"),
        (vec!["--foo".into(), l()], "unknown flag --foo"),
        (vec!["--".into(), l()], "unknown flag --"),
        (vec!["--trace=x".into(), l()], "unknown flag --trace=x"),
    ] {
        assert_eq!(run(&args, &dir.join("traces")), usage(why), "{args:?}");
    }
}

#[test]
fn a_log_that_does_not_read_is_exit_1() {
    let dir = tree("log-unreadable");
    let r = run(&[dir.join("absent.log").into()], &dir.join("traces"));
    assert_eq!((r.stdout.as_str(), r.code), ("", 1));
    assert!(r.stderr.starts_with(&format!(
        "gresil-trace-diff: {}: ",
        dir.join("absent.log").display()
    )));
}

/// A capture made of the tree's own plasticity-off traces, a boot line
/// first and the end line last; the tree's trace files, and how many
/// cases the capture holds.
fn the_tree_s_replay() -> (String, Vec<PathBuf>, usize) {
    let traces = Path::new(env!("CARGO_MANIFEST_DIR")).join("../gresil/tests/traces");
    let mut files: Vec<PathBuf> = fs::read_dir(&traces)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|x| x == "trace"))
        .collect();
    files.sort();
    let mut text = String::from("ESP-ROM:esp32c3-api1-20210207\r\n");
    let mut n = 0;
    for f in &files {
        let body = fs::read_to_string(f).unwrap();
        if body
            .lines()
            .next()
            .is_some_and(|h| h.contains(" plasticity=off "))
        {
            text.push_str(&body);
            n += 1;
        }
    }
    text.push_str(END);
    text.push('\n');
    (text, files, n)
}

/// The binary, on a capture made of the tree's own plasticity-off traces.
#[test]
fn the_binary_reads_the_tree_s_traces() {
    let (text, files, n) = the_tree_s_replay();
    let dir = scratch("binary");
    let log = dir.join("capture.log");
    fs::write(&log, &text).unwrap();
    let bin = env!("CARGO_BIN_EXE_gresil-trace-diff");
    let out = Command::new(bin).arg(&log).output().unwrap();
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&out.stdout)
    );
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(
        n >= 1 && stdout.ends_with(&format!("\n{n} cases, 0 red\n")),
        "{stdout}"
    );
    assert_eq!(
        stdout
            .lines()
            .filter(|l| l.contains(": identical, "))
            .count(),
        n
    );
    assert!(out.stderr.is_empty());

    let given = dir.join("mine.trace");
    let first = fs::read_to_string(&files[0]).unwrap();
    let renamed = first.replacen(" case=", " case=mine-", 1);
    fs::write(&given, &renamed).unwrap();
    fs::write(&log, text.replacen(END, &format!("{renamed}{END}"), 1)).unwrap();
    let out = Command::new(bin)
        .arg("--trace")
        .arg(&given)
        .arg(&log)
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(0));
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(
        stdout.contains("\nmine (given): identical, ")
            && stdout.ends_with(&format!("\n{} cases, 0 red\n", n + 1)),
        "{stdout}"
    );

    let out = Command::new(bin).output().unwrap();
    assert_eq!(out.status.code(), Some(2));
    assert!(out.stdout.is_empty());
    assert_eq!(
        String::from_utf8(out.stderr).unwrap(),
        format!("gresil-trace-diff: one LOG\n{USAGE}\n")
    );
}

/// Run by cargo for another place than the one it was built in (a target
/// dir two trees share, a tree moved with its target dir), the binary
/// refuses: one line that names both and says to touch `lib.rs`,
/// nothing on stdout, exit 1.
#[test]
fn the_binary_refuses_a_run_for_another_place() {
    let (text, _, _) = the_tree_s_replay();
    let dir = scratch("another-place");
    let theirs = dir.join("crates/gresil-trace-diff");
    fs::create_dir_all(&theirs).unwrap();
    let log = dir.join("capture.log");
    fs::write(&log, &text).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_gresil-trace-diff"))
        .env("CARGO_MANIFEST_DIR", &theirs)
        .arg(&log)
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert!(out.stdout.is_empty());
    assert_eq!(
        String::from_utf8(out.stderr).unwrap(),
        format!(
            "gresil-trace-diff: built in {}, run by cargo for {}: its traces are another tree's; touch {} and run again\n",
            env!("CARGO_MANIFEST_DIR"),
            theirs.display(),
            theirs.join("src/lib.rs").display()
        )
    );
}

/// Run by cargo for a place that does not exist, the binary refuses as
/// for another place. In a cargo flow the side gone is the other one, the
/// place a tree moved with its target dir was built in; a test's own
/// build place exists, so this one names a gone place to run for.
#[test]
fn the_binary_refuses_a_run_for_a_place_that_does_not_exist() {
    let (text, _, _) = the_tree_s_replay();
    let dir = scratch("a-place-gone");
    let gone = dir.join("gone/crates/gresil-trace-diff");
    let log = dir.join("capture.log");
    fs::write(&log, &text).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_gresil-trace-diff"))
        .env("CARGO_MANIFEST_DIR", &gone)
        .arg(&log)
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert!(out.stdout.is_empty());
    assert_eq!(
        String::from_utf8(out.stderr).unwrap(),
        format!(
            "gresil-trace-diff: built in {}, run by cargo for {}: its traces are another tree's; touch {} and run again\n",
            env!("CARGO_MANIFEST_DIR"),
            gone.display(),
            gone.join("src/lib.rs").display()
        )
    );
}

/// The binary's own place through a symlinked tree is the same place.
#[cfg(unix)]
#[test]
fn the_binary_reads_its_own_tree_through_a_symlink() {
    let (text, _, n) = the_tree_s_replay();
    let dir = scratch("symlinked-tree");
    let link = dir.join("tree");
    std::os::unix::fs::symlink(Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."), &link).unwrap();
    let log = dir.join("capture.log");
    fs::write(&log, &text).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_gresil-trace-diff"))
        .env("CARGO_MANIFEST_DIR", link.join("crates/gresil-trace-diff"))
        .arg(&log)
        .output()
        .unwrap();
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(
        stdout.ends_with(&format!("\n{n} cases, 0 red\n")),
        "{stdout}"
    );
}

/// Run by its path, `CARGO_MANIFEST_DIR` unset, the binary reads the tree
/// it was built in.
#[test]
fn the_binary_run_by_its_path_reads_the_built_in_tree() {
    let (text, _, n) = the_tree_s_replay();
    let dir = scratch("by-its-path");
    let log = dir.join("capture.log");
    fs::write(&log, &text).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_gresil-trace-diff"))
        .env_remove("CARGO_MANIFEST_DIR")
        .arg(&log)
        .output()
        .unwrap();
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(
        stdout.ends_with(&format!("\n{n} cases, 0 red\n")),
        "{stdout}"
    );
}
