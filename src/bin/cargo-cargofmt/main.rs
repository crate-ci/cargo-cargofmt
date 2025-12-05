use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::hash::{Hash, Hasher};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::str;

use cargo_metadata::Edition;
use cargo_metadata::Metadata;
use cargo_metadata::Package;
use cargo_metadata::PackageId;
use cargo_metadata::TargetKind;
use clap::CommandFactory;
use clap::Parser;

const SUCCESS: i32 = 0;
const FAILURE: i32 = 1;

#[derive(Parser)]
#[command(name = "cargo")]
#[command(bin_name = "cargo")]
#[command(version)]
#[command(styles = clap_cargo::style::CLAP_STYLING)]
enum CargoOpts {
    CargoFmt(Opts),
}

#[derive(clap::Args)]
#[command(version)]
struct Opts {
    /// Specify path to Cargo.toml
    #[arg(long, value_name = "TOML")]
    manifest_path: Option<PathBuf>,

    /// Specify package to format
    #[arg(short, long = "packages", value_name = "SPEC")]
    packages: Vec<String>,

    /// Format all packages, and also their local path-based dependencies
    #[arg(long = "all")]
    format_all: bool,

    /// Run rustfmt in check mode
    #[arg(long)]
    check: bool,
}

fn main() {
    let exit_status = execute();
    io::stdout().flush().unwrap();
    std::process::exit(exit_status);
}

fn execute() -> i32 {
    let opts = CargoOpts::parse();
    let CargoOpts::CargoFmt(opts) = opts;

    let strategy = CargoFmtStrategy::from_opts(&opts);

    if let Some(manifest_path) = opts.manifest_path.clone() {
        if manifest_path.file_name() != Some(std::ffi::OsStr::new("Cargo.toml")) {
            print_usage_to_stderr("the manifest-path must be a path to a Cargo.toml file");
            return FAILURE;
        }
        handle_command_status(format_crates(&strategy, opts.check, Some(&manifest_path)))
    } else {
        handle_command_status(format_crates(&strategy, opts.check, None))
    }
}

fn print_usage_to_stderr(reason: &str) {
    eprintln!("{reason}");
    let app = CargoOpts::command();
    let help = app.after_help("").render_help();
    eprintln!("{help}");
}

fn handle_command_status(status: Result<i32, io::Error>) -> i32 {
    match status {
        Err(e) => {
            print_usage_to_stderr(&e.to_string());
            FAILURE
        }
        Ok(status) => status,
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum CargoFmtStrategy {
    /// Format every packages and dependencies.
    All,
    /// Format packages that are specified by the command line argument.
    Some(Vec<String>),
    /// Format the root packages only.
    Root,
}

impl CargoFmtStrategy {
    fn from_opts(opts: &Opts) -> CargoFmtStrategy {
        match (opts.format_all, opts.packages.is_empty()) {
            (false, true) => CargoFmtStrategy::Root,
            (true, _) => CargoFmtStrategy::All,
            (false, false) => CargoFmtStrategy::Some(opts.packages.clone()),
        }
    }
}

/// Target uses a `path` field for equality and hashing.
#[derive(Debug)]
pub struct Target {
    /// A path to the main source file of the target.
    path: PathBuf,
    /// A kind of target (e.g., lib, bin, example, ...).
    #[allow(unused)]
    kind: TargetKind,
    /// Rust edition for this target.
    #[allow(unused)]
    edition: Edition,
}

impl Target {
    pub fn from_target(target: &cargo_metadata::Target) -> Self {
        let path = PathBuf::from(&target.src_path);
        let canonicalized = fs::canonicalize(&path).unwrap_or(path);

        Target {
            path: canonicalized,
            kind: target.kind[0].clone(),
            edition: target.edition,
        }
    }
}

impl PartialEq for Target {
    fn eq(&self, other: &Target) -> bool {
        self.path == other.path
    }
}

impl PartialOrd for Target {
    fn partial_cmp(&self, other: &Target) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Target {
    fn cmp(&self, other: &Target) -> Ordering {
        self.path.cmp(&other.path)
    }
}

impl Eq for Target {}

impl Hash for Target {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.path.hash(state);
    }
}

fn format_crates(
    strategy: &CargoFmtStrategy,
    check: bool,
    manifest_path: Option<&Path>,
) -> Result<i32, io::Error> {
    let metadata = get_cargo_metadata(manifest_path)?;
    let packages = get_packages(strategy, manifest_path, &metadata)?;
    let _targets = to_targets(&packages);

    let mut errors = 0;
    for package in packages.values() {
        if let Err(err) = format_crate(check, package) {
            if let Some(err) = err {
                anstream::eprintln!("{err}");
            }
            errors += 1;
        }
    }

    if !rustfmt(strategy, check, manifest_path) {
        errors += 1;
    }

    let code = if 0 < errors { FAILURE } else { SUCCESS };
    Ok(code)
}

fn rustfmt(strategy: &CargoFmtStrategy, check: bool, manifest_path: Option<&Path>) -> bool {
    let cargo = env::var_os("CARGO").unwrap_or_else(|| std::ffi::OsString::from("cargo"));
    let mut cmd = std::process::Command::new(cargo);
    cmd.arg("fmt");
    if check {
        cmd.arg("--check");
    }
    match strategy {
        CargoFmtStrategy::All => {
            cmd.arg("--all");
        }
        CargoFmtStrategy::Some(p) => {
            for p in p {
                cmd.arg("--package").arg(p);
            }
        }
        CargoFmtStrategy::Root => {}
    }
    if let Some(manifest_path) = manifest_path {
        cmd.arg("--manifest-path").arg(manifest_path);
    }

    cmd.status().map(|s| s.success()).unwrap_or(false)
}

fn format_crate(check: bool, package: &Package) -> Result<(), Option<io::Error>> {
    let config = cargo_cargofmt::config::load_config(package.manifest_path.as_std_path())?;

    if config.disable_all_formatting {
        return Ok(());
    }

    let raw_input_text = cargo_util::paths::read(package.manifest_path.as_std_path())
        .map_err(io::Error::other)
        .map_err(Some)?;
    let mut input = raw_input_text.clone();

    // Normalize for easier manipulation
    cargo_cargofmt::formatting::apply_newline_style(
        cargo_cargofmt::config::options::NewlineStyle::Unix,
        &mut input,
        &raw_input_text,
    );

    let document = input
        .parse::<toml_edit::DocumentMut>()
        .map_err(io::Error::other)
        .map_err(Some)?;

    let mut formatted = document.to_string();

    cargo_cargofmt::formatting::apply_newline_style(
        config.newline_style,
        &mut formatted,
        &raw_input_text,
    );

    if input != formatted {
        if check {
            let name = package.manifest_path.as_std_path();
            let name = name.to_string_lossy();
            let mut stream = String::new();
            snapbox::report::write_diff(
                &mut stream,
                &input.into(),
                &formatted.into(),
                Some(&name),
                None,
                snapbox::report::Palette::color(),
            )
            .map_err(io::Error::other)
            .map_err(Some)?;
            anstream::println!("{stream}");
        } else {
            cargo_util::paths::write_atomic(package.manifest_path.as_std_path(), formatted)
                .map_err(io::Error::other)
                .map_err(Some)?;
        }
    }

    Ok(())
}

/// Based on the specified `CargoFmtStrategy`, returns a set of main source files.
fn get_packages<'m>(
    strategy: &CargoFmtStrategy,
    manifest_path: Option<&Path>,
    metadata: &'m Metadata,
) -> Result<BTreeMap<PackageId, &'m Package>, io::Error> {
    let mut packages = BTreeMap::new();

    match *strategy {
        CargoFmtStrategy::Root => get_packages_root_only(manifest_path, metadata, &mut packages)?,
        CargoFmtStrategy::All => {
            get_packages_recursive(metadata, &mut packages)?;
        }
        CargoFmtStrategy::Some(ref hitlist) => {
            get_packages_with_hitlist(metadata, hitlist, &mut packages)?;
        }
    }

    if packages.is_empty() {
        Err(io::Error::other("Failed to find targets".to_owned()))
    } else {
        Ok(packages)
    }
}

fn get_packages_root_only<'m>(
    manifest_path: Option<&Path>,
    metadata: &'m Metadata,
    packages: &mut BTreeMap<PackageId, &'m Package>,
) -> Result<(), io::Error> {
    let workspace_root_path = PathBuf::from(&metadata.workspace_root).canonicalize()?;
    let (in_workspace_root, current_dir_manifest) = if let Some(target_manifest) = manifest_path {
        (
            workspace_root_path == target_manifest,
            target_manifest.canonicalize()?,
        )
    } else {
        let current_dir = env::current_dir()?.canonicalize()?;
        (
            workspace_root_path == current_dir,
            current_dir.join("Cargo.toml"),
        )
    };

    packages.extend(
        metadata
            .packages
            .iter()
            .filter(|p| {
                in_workspace_root
                    || PathBuf::from(&p.manifest_path)
                        .canonicalize()
                        .unwrap_or_default()
                        == current_dir_manifest
            })
            .map(|p| (p.id.clone(), p)),
    );

    Ok(())
}

fn get_packages_recursive<'m>(
    metadata: &'m Metadata,
    packages: &mut BTreeMap<PackageId, &'m Package>,
) -> Result<(), io::Error> {
    for package in &metadata.packages {
        if packages.insert(package.id.clone(), package).is_none() {
            continue;
        }

        // Look for local dependencies using information available since cargo v1.51
        // It's theoretically possible someone could use a newer version of rustfmt with
        // a much older version of `cargo`, but we don't try to explicitly support that scenario.
        // If someone reports an issue with path-based deps not being formatted, be sure to
        // confirm their version of `cargo` (not `cargo-fmt`) is >= v1.51
        // https://github.com/rust-lang/cargo/pull/8994
        for dependency in &package.dependencies {
            let Some(path) = dependency.path.as_ref() else {
                continue;
            };

            let manifest_path = path.join("Cargo.toml");
            if manifest_path.exists()
                && !metadata
                    .packages
                    .iter()
                    .any(|p| p.manifest_path.eq(&manifest_path))
            {
                get_packages_recursive(metadata, packages)?;
            }
        }
    }

    Ok(())
}

fn get_packages_with_hitlist<'m>(
    metadata: &'m Metadata,
    hitlist: &[String],
    packages: &mut BTreeMap<PackageId, &'m Package>,
) -> Result<(), io::Error> {
    let mut workspace_hitlist: BTreeSet<&String> = BTreeSet::from_iter(hitlist);

    for package in &metadata.packages {
        if workspace_hitlist.remove(&package.name) {
            packages.insert(package.id.clone(), package);
        }
    }

    if workspace_hitlist.is_empty() {
        Ok(())
    } else {
        let package = workspace_hitlist.iter().next().unwrap();
        Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("package `{package}` is not a member of the workspace"),
        ))
    }
}

fn to_targets(packages: &BTreeMap<PackageId, &Package>) -> BTreeSet<Target> {
    let mut targets = BTreeSet::new();
    for package in packages.values() {
        for target in &package.targets {
            targets.insert(Target::from_target(target));
        }
    }
    targets
}

fn get_cargo_metadata(manifest_path: Option<&Path>) -> Result<Metadata, io::Error> {
    let mut cmd = cargo_metadata::MetadataCommand::new();
    cmd.no_deps();
    if let Some(manifest_path) = manifest_path {
        cmd.manifest_path(manifest_path);
    }
    cmd.other_options(vec![String::from("--offline")]);

    match cmd.exec() {
        Ok(metadata) => Ok(metadata),
        Err(_) => {
            cmd.other_options(vec![]);
            match cmd.exec() {
                Ok(metadata) => Ok(metadata),
                Err(error) => Err(io::Error::other(error.to_string())),
            }
        }
    }
}
