//! `cargo xtask sprt`: plays the working tree against an earlier revision until a
//! sequential probability ratio test decides whether it is stronger.
//!
//! The games are played by fastchess, which is only run as a separate program.

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};

const USAGE: &str = "\
usage:
  cargo xtask sprt --base <revision or engine file> [options]

options:
  --bounds <name>      early = [0, 10] (default), standard = [0, 5], late = [0, 3],
                       regress = [-5, 0]
  --tc <time control>  default 8+0.08
  --concurrency <n>    games played at once; default is one less than half the
                       logical processors
  --fastchess <path>   default: the FASTCHESS environment variable, then `fastchess`
  --out <dir>          where builds and the PGN go; default <temp>/bear-sprt

The working tree is built and played as `new`. A revision given as --base is exported,
built once and kept in the output directory.";

const BOOK: &str = "books/bear-8ply.epd";

struct Options {
    base: String,
    elo: (i32, i32),
    tc: String,
    concurrency: usize,
    fastchess: String,
    out: PathBuf,
}

fn parse(args: &[String]) -> Result<Options, String> {
    let default_concurrency =
        std::thread::available_parallelism().map_or(1, |n| (n.get() / 2).saturating_sub(1).max(1));
    let mut options = Options {
        base: String::new(),
        elo: (0, 10),
        tc: "8+0.08".to_string(),
        concurrency: default_concurrency,
        fastchess: std::env::var("FASTCHESS").unwrap_or_else(|_| "fastchess".to_string()),
        out: std::env::temp_dir().join("bear-sprt"),
    };
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        let value = args
            .next()
            .ok_or_else(|| format!("{arg} needs a value\n\n{USAGE}"))?;
        match arg.as_str() {
            "--base" => options.base = value.clone(),
            "--bounds" => {
                options.elo = match value.as_str() {
                    "early" => (0, 10),
                    "standard" => (0, 5),
                    "late" => (0, 3),
                    "regress" => (-5, 0),
                    other => return Err(format!("unknown bounds {other}\n\n{USAGE}")),
                }
            }
            "--tc" => options.tc = value.clone(),
            "--concurrency" => {
                options.concurrency = value
                    .parse::<usize>()
                    .ok()
                    .filter(|&n| n > 0)
                    .ok_or_else(|| format!("bad concurrency {value}"))?
            }
            "--fastchess" => options.fastchess = value.clone(),
            "--out" => options.out = PathBuf::from(value),
            other => return Err(format!("unknown option {other}\n\n{USAGE}")),
        }
    }
    if options.base.is_empty() {
        return Err(USAGE.to_string());
    }
    Ok(options)
}

fn engine_name() -> String {
    format!("bear{}", std::env::consts::EXE_SUFFIX)
}

/// Runs `command` to completion and fails unless it succeeds.
fn run(command: &mut Command, what: &str) -> Result<(), String> {
    let status = command
        .status()
        .map_err(|e| format!("cannot {what}: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("failed to {what} ({status})"))
    }
}

fn copy(from: &Path, to: &Path) -> Result<(), String> {
    std::fs::copy(from, to)
        .map(|_| ())
        .map_err(|e| format!("cannot copy {} to {}: {e}", from.display(), to.display()))
}

fn create_dir(dir: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))
}

/// Builds the working tree and copies the engine aside, so that later builds do not
/// replace it under a running match.
fn build_new(root: &Path, out: &Path) -> Result<PathBuf, String> {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
    run(
        Command::new(cargo)
            .args(["build", "--release", "-p", "bear-uci"])
            .current_dir(root),
        "build the working tree",
    )?;
    // This program is built into the same directory as the engine.
    let built = std::env::current_exe()
        .map_err(|e| format!("cannot locate the build directory: {e}"))?
        .with_file_name(engine_name());
    let dir = out.join("new");
    create_dir(&dir)?;
    let engine = dir.join(engine_name());
    copy(&built, &engine)?;
    Ok(engine)
}

/// The engine at `revision`, built from an export of that commit unless an earlier
/// run already left it in `out`.
fn build_base(root: &Path, out: &Path, revision: &str) -> Result<PathBuf, String> {
    let output = Command::new("git")
        .args(["rev-parse", "--verify", "--short=12"])
        .arg(format!("{revision}^{{commit}}"))
        .current_dir(root)
        .output()
        .map_err(|e| format!("cannot run git: {e}"))?;
    if !output.status.success() {
        return Err(format!("{revision} is neither a file nor a revision"));
    }
    let hash = String::from_utf8_lossy(&output.stdout).trim().to_string();

    let dir = out.join(&hash);
    let engine = dir.join(engine_name());
    if engine.is_file() {
        return Ok(engine);
    }

    let source = dir.join("src");
    let target = dir.join("target");
    create_dir(&source)?;
    let mut archive = Command::new("git")
        .args(["archive", "--format=tar", &hash])
        .current_dir(root)
        .stdout(Stdio::piped())
        .spawn()
        .map_err(|e| format!("cannot run git archive: {e}"))?;
    let exported = archive.stdout.take().expect("stdout was piped");
    run(
        Command::new("tar")
            .args(["-x", "-f", "-", "-C"])
            .arg(&source)
            .stdin(exported),
        "unpack the base revision",
    )?;
    let status = archive.wait().map_err(|e| format!("git archive: {e}"))?;
    if !status.success() {
        return Err(format!("git archive failed ({status})"));
    }

    // Through the toolchain proxy, so that the revision's own pinned toolchain is used.
    run(
        Command::new("cargo")
            .args(["build", "--release", "-p", "bear-uci", "--target-dir"])
            .arg(&target)
            .current_dir(&source)
            .env_remove("CARGO")
            .env_remove("RUSTUP_TOOLCHAIN"),
        "build the base revision",
    )?;
    copy(&target.join("release").join(engine_name()), &engine)?;
    // Only the engine is worth keeping.
    let _ = std::fs::remove_dir_all(&source);
    let _ = std::fs::remove_dir_all(&target);
    Ok(engine)
}

fn sprt(args: &[String]) -> Result<ExitCode, String> {
    let options = parse(args)?;
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask lives inside the workspace");
    let book = root.join(BOOK);
    create_dir(&options.out)?;
    let out = options
        .out
        .canonicalize()
        .map_err(|e| format!("cannot resolve {}: {e}", options.out.display()))?;

    let new = build_new(root, &out)?;
    let base = if Path::new(&options.base).is_file() {
        PathBuf::from(&options.base)
            .canonicalize()
            .map_err(|e| format!("cannot resolve {}: {e}", options.base))?
    } else {
        build_base(root, &out, &options.base)?
    };

    let (elo0, elo1) = options.elo;
    println!(
        "new:  {}\nbase: {}\nSPRT [{elo0}, {elo1}] at {}, {} games at once",
        new.display(),
        base.display(),
        options.tc,
        options.concurrency
    );

    let status = Command::new(&options.fastchess)
        .args(["-engine", &format!("cmd={}", new.display()), "name=new"])
        .args(["-engine", &format!("cmd={}", base.display()), "name=base"])
        .args(["-each", &format!("tc={}", options.tc), "proto=uci"])
        .arg("option.Hash=16")
        .args(["-openings", &format!("file={}", book.display())])
        .args(["format=epd", "order=random"])
        .args(["-rounds", "20000", "-games", "2", "-repeat", "-recover"])
        .args(["-concurrency", &options.concurrency.to_string()])
        .args(["-sprt", &format!("elo0={elo0}"), &format!("elo1={elo1}")])
        .args(["alpha=0.05", "beta=0.05"])
        .args(["-pgnout", "file=sprt.pgn"])
        .current_dir(&out)
        .status()
        .map_err(|e| format!("cannot run {}: {e}", options.fastchess))?;
    Ok(if status.success() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    })
}

pub fn run_task(args: &[String]) -> ExitCode {
    sprt(args).unwrap_or_else(|message| {
        eprintln!("{message}");
        ExitCode::FAILURE
    })
}
