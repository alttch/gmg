// TODO git-shell-commands
use bmart_derive::EnumStr;
use clap::{Parser, Subcommand};
use colored::Colorize;
use lazy_static::lazy_static;
use std::fmt;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::atomic;

mod repo;
mod user;

use repo::Repository;
use user::User;

impl std::error::Error for ErrorKind {}

const AUTHOR: &str = "Serhij S. / Bohemia Automation";
const VERSION: &str = env!("CARGO_PKG_VERSION");

const GROUP_PFX: &str = "g_";

const GIT_USER: &str = "git";

const MAIN_BRANCH: &str = "main";
const PROTECTED_BRANCHES: [&str; 1] = [MAIN_BRANCH];

static VERBOSE: atomic::AtomicBool = atomic::AtomicBool::new(false);

type DResult<T> = Result<T, Box<dyn std::error::Error>>;

#[macro_export]
macro_rules! failed {
    ($err: expr) => {
        Err(Box::new(ErrorKind::Failed($err)))
    };
}

#[macro_export]
macro_rules! err {
    ($msg: expr) => {
        eprintln!("{}", $msg.red());
    };
}

#[inline]
fn sh(cmd: &str) -> DResult<String> {
    sh_cmd(cmd, true)
}

#[inline]
fn sh_any(cmd: &str) -> DResult<String> {
    sh_cmd(cmd, false)
}

fn sh_cmd(cmd: &str, check_exit_code: bool) -> DResult<String> {
    if VERBOSE.load(atomic::Ordering::SeqCst) {
        println!("> {}", cmd.dimmed().bold());
    }
    let output = std::process::Command::new("sh")
        .args(["-c", cmd])
        .output()?;
    let code = output.status.code().unwrap_or(-1);
    let out = std::str::from_utf8(&output.stdout)?;
    if check_exit_code && code != 0 {
        eprintln!("{}", std::str::from_utf8(&output.stderr)?.red());
        failed!(format!("{}\nprocess exit code: {}", out, code))
    } else {
        Ok(out.to_owned())
    }
}

pub fn chmod<P: AsRef<Path>>(path: P, permissions: u32) -> DResult<()> {
    fs::set_permissions(path, fs::Permissions::from_mode(permissions))?;
    Ok(())
}

lazy_static! {
    static ref GIT_PATH: PathBuf = Path::new("/git").to_owned();
    static ref HOME_PATH: PathBuf = Path::new("/home").to_owned();
}

#[derive(Parser)]
struct RepoRenameParams {
    #[clap()]
    repository: Repository,
    #[clap()]
    new_repository: Repository,
}

#[derive(Parser)]
struct RepoBranchesParams {
    #[clap()]
    repository: Repository,
    #[clap(short = 's', long = "short")]
    short: bool,
}

#[derive(Parser)]
struct RepoParams {
    #[clap()]
    repository: Repository,
}

#[derive(Parser)]
struct RepoRciParams {
    #[clap()]
    repository: Repository,
    #[clap()]
    branch: String,
    #[clap(subcommand)]
    command: RciCommand,
}

#[derive(Parser)]
enum RciCommand {
    Set(RciSetParams),
    Unset,
}

#[derive(Parser)]
struct RciSetParams {
    #[clap(help = "RCI server top URL")]
    rci_url: String,
    #[clap()]
    rci_job: String,
    #[clap()]
    rci_secret: String,
}

#[derive(Parser)]
struct RepoBranchParams {
    #[clap()]
    repository: Repository,
    #[clap()]
    branch: String,
}

#[derive(Parser)]
struct RepoCreateParams {
    #[clap()]
    repository: Repository,
    #[clap(long = "init-only")]
    init_only: bool,
    #[clap(short = 'D', long = "description")]
    description: Option<String>,
}

#[derive(Parser)]
struct RepoSetParams {
    #[clap()]
    repository: Repository,
    #[clap()]
    property: RepoProp,
    #[clap()]
    value: String,
}

#[derive(Clone, Copy, Parser, EnumStr)]
#[enumstr(rename_all = "lowercase")]
enum RepoProp {
    Description,
}

#[derive(Parser)]
#[command(author = AUTHOR, version)]
struct Args {
    #[clap(short = 'v', long = "verbose")]
    verbose: bool,
    #[clap(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum RepoCommand {
    Archive(RepoParams),
    Branches(RepoBranchesParams),
    Check(RepoParams),
    Cleanup(RepoParams),
    Create(RepoCreateParams),
    Destroy(RepoParams),
    Fix(RepoParams),
    Set(RepoSetParams),
    Info(RepoParams),
    List(ListParams),
    Protect(RepoBranchParams),
    Rename(RepoRenameParams),
    Unprotect(RepoBranchParams),
    Users(RepoParams),
    Rci(RepoRciParams),
}

#[derive(Subcommand)]
enum UserCommand {
    Create(UserCreateParams),
    Destroy(UserParams),
    Grant(UserRepoParams),
    List(ListParams),
    Repos(UserReposParams),
    Revoke(UserRepoParams),
    Update(UserParams),
}

#[derive(Subcommand)]
enum MaintainerCommand {
    Set(UserRepoParams),
    Unset(UserRepoParams),
}

#[derive(Parser)]
struct UserCreateParams {
    #[clap(name = "login")]
    user: User,
    #[clap(help = "First/last name (quoted)")]
    name: String,
    #[clap(help = "SSH public key file, '-' for stdin")]
    key_file: String,
}

#[derive(Parser)]
struct ListParams {
    #[clap(short = 's', long = "short")]
    short: bool,
}

#[derive(Parser)]
struct UserParams {
    #[clap(name = "login")]
    user: User,
}

#[derive(Parser)]
struct UserReposParams {
    #[clap(name = "login")]
    user: User,
    #[clap(short = 's', long = "short")]
    short: bool,
}

#[derive(Parser)]
struct UserRepoParams {
    #[clap(name = "login")]
    user: User,
    #[clap()]
    repository: Repository,
}

#[derive(Subcommand)]
enum Command {
    #[clap(subcommand)]
    Repo(RepoCommand),
    #[clap(subcommand)]
    User(UserCommand),
    #[clap(subcommand)]
    Maintainer(MaintainerCommand),
}

#[derive(Debug)]
pub enum ErrorKind {
    Failed(String),
}

impl fmt::Display for ErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                ErrorKind::Failed(msg) => msg,
            }
        )
    }
}

fn repo_cmd(command: RepoCommand) -> DResult<()> {
    match command {
        RepoCommand::Archive(params) => params.repository.archive()?,
        RepoCommand::Branches(params) => {
            for r in params.repository.branches()? {
                println!("{}", r.yellow());
            }
        }
        RepoCommand::Check(params) => params.repository.check()?,
        RepoCommand::Cleanup(params) => params.repository.cleanup()?,
        RepoCommand::Create(params) => {
            params
                .repository
                .create(params.init_only, params.description.as_deref())?;
        }
        RepoCommand::Set(params) => match params.property {
            RepoProp::Description => params.repository.set_description(Some(&params.value))?,
        },
        RepoCommand::Destroy(params) => params.repository.destroy()?,
        RepoCommand::Fix(params) => params.repository.fix(true)?,
        RepoCommand::Info(params) => params.repository.print_info()?,
        RepoCommand::List(params) => Repository::print_all(params.short)?,
        RepoCommand::Protect(params) => {
            params.repository.protect(&params.branch)?;
            println!(
                "Repository {} branch {} has been {}",
                params.repository.name_colored(),
                params.branch.yellow(),
                "protected".green().bold()
            );
        }
        RepoCommand::Rename(params) => params.repository.rename(&params.new_repository)?,
        RepoCommand::Unprotect(params) => {
            params.repository.unprotect(&params.branch)?;
            println!(
                "Repository {} branch {} has been {}",
                params.repository.name_colored(),
                params.branch.yellow(),
                "unprotected".red().bold()
            );
        }
        RepoCommand::Users(params) => {
            for user in params.repository.users()? {
                println!("{}", user.login_colored());
            }
        }
        RepoCommand::Rci(params) => {
            let branch = params.branch;
            match params.command {
                RciCommand::Set(c) => {
                    let rci_job = c.rci_job;
                    let rci_secret = c.rci_secret;
                    let mut url = c.rci_url.as_str();
                    while url.ends_with('/') {
                        url = &url[..url.len() - 1];
                    }
                    let rci_trigger_url = format!("{url}/job/{rci_job}/trigger");
                    params
                        .repository
                        .set(&format!("hooks.branch.{branch}.rci.url"), &rci_trigger_url)?;
                    params
                        .repository
                        .set(&format!("hooks.branch.{branch}.rci.secret"), &rci_secret)?;
                    println!(
                        "RCI config {} for {} branch {}, trigger URL: {}",
                        "SET".green().bold(),
                        params.repository.name_colored(),
                        branch.yellow(),
                        rci_trigger_url
                    );
                }
                RciCommand::Unset => {
                    params
                        .repository
                        .unset(&format!("hooks.branch.{branch}.rci.url"))?;
                    params
                        .repository
                        .unset(&format!("hooks.branch.{branch}.rci.secret"))?;
                    println!(
                        "RCI config {} for {} branch {}",
                        "UNSET".yellow().bold(),
                        params.repository.name_colored(),
                        branch.yellow()
                    );
                }
            }
        }
    }
    Ok(())
}

fn user_cmd(command: UserCommand) -> DResult<()> {
    match command {
        UserCommand::Create(params) => params.user.create(&params.name, &params.key_file)?,
        UserCommand::Destroy(params) => params.user.destroy()?,
        UserCommand::Grant(params) => params.user.grant(&params.repository)?,
        UserCommand::List(params) => User::print_all(params.short)?,
        UserCommand::Repos(params) => {
            for r in params.user.repos()? {
                if params.short {
                    println!("{}", r.name_colored(),);
                } else {
                    println!(
                        "{} ({})",
                        r.name_colored(),
                        r.read_description()?.unwrap_or_default()
                    );
                }
            }
        }
        UserCommand::Revoke(params) => params.user.revoke(&params.repository)?,
        UserCommand::Update(params) => params.user.update()?,
    }
    Ok(())
}

fn maintainer_cmd(command: MaintainerCommand) -> DResult<()> {
    match command {
        MaintainerCommand::Set(m) => m.user.maintainer_set(&m.repository)?,
        MaintainerCommand::Unset(m) => m.user.maintainer_unset(&m.repository)?,
    }
    Ok(())
}

fn main() -> DResult<()> {
    let args = Args::parse();
    VERBOSE.store(args.verbose, atomic::Ordering::SeqCst);
    match args.command {
        Command::Repo(c) => repo_cmd(c)?,
        Command::User(c) => user_cmd(c)?,
        Command::Maintainer(c) => maintainer_cmd(c)?,
    }
    Ok(())
}
