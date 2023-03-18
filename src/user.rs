use crate::repo::Repository;
use crate::{chmod, sh, sh_any, DResult, ErrorKind, GIT_PATH, GROUP_PFX, HOME_PATH};
use bmart_derive::Sorting;
use colored::{ColoredString, Colorize};
use std::env::set_current_dir;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::str::FromStr;

#[derive(Clone, Sorting)]
#[sorting(id = "login")]
pub struct User {
    login: String,
    home: PathBuf,
}

impl FromStr for User {
    type Err = ErrorKind;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut path = HOME_PATH.clone();
        path.push(s);
        Ok(Self {
            login: s.to_owned(),
            home: path,
        })
    }
}

impl User {
    pub fn login(&self) -> &str {
        &self.login
    }
    pub fn login_colored(&self) -> ColoredString {
        self.login.yellow()
    }
    pub fn home(&self) -> &Path {
        &self.home
    }
    pub fn chdir(&self) -> DResult<()> {
        set_current_dir(&self.home)?;
        Ok(())
    }
    pub fn exists(&self) -> DResult<()> {
        sh(&format!(r#"id "{}""#, self.login))?;
        Ok(())
    }
    pub fn create(&self, name: &str, key_file: &str) -> DResult<()> {
        let git_shell = which::which("git-shell")?;
        let key = if key_file == "-" {
            println!("Paste a public SSH key here, Ctrl+C to abort");
            let mut stdin = std::io::stdin();
            let mut key = String::new();
            stdin.read_to_string(&mut key)?;
            key
        } else {
            fs::read_to_string(key_file)?
        };
        sh(&format!(
            r#"useradd -m --shell "{}" "{}""#,
            git_shell.to_string_lossy(),
            self.login()
        ))?;
        sh(&format!(r#"chfn -f "{}" "{}""#, name, self.login()))?;
        chmod(self.home(), 0o700)?;
        self.chdir()?;
        fs::create_dir_all(".ssh")?;
        fs::write(".ssh/authorized_keys", key)?;
        chmod(".ssh", 0o700)?;
        sh(&format!(r#"chown -R "{}" .ssh"#, self.login()))?;
        self.update_cgit()?;
        println!("User created: {}", self.login_colored());
        Ok(())
    }
    pub fn update(&self) -> DResult<()> {
        self.update_cgit()?;
        Ok(())
    }
    pub fn update_cgit(&self) -> DResult<()> {
        let cgitrc = fs::read_to_string("/etc/cgitrc").unwrap_or_default();
        let mut config = Vec::new();
        for line in cgitrc.lines() {
            if !line.starts_with("repo.") {
                config.push(line.to_owned());
            }
        }
        for repo in self.repos()? {
            config.push(format!("repo.url={}", repo.name()));
            config.push(format!("repo.path={}", repo.path_as_str()));
            if let Some(desc) = repo.read_description()? {
                config.push(format!("repo.desc={}", desc));
            }
        }
        config.push(String::new());
        let mut path = GIT_PATH.clone();
        path.push(format!(".config/cgit/{}.cgitrc", self.login()));
        fs::write(path, config.join("\n"))?;
        Ok(())
    }
    pub fn repos(&self) -> DResult<Vec<Repository>> {
        self.exists()?;
        let out = sh(&format!(r#"groups "{}""#, self.login()))?;
        let mut result = Vec::new();
        if let Some(groups) = out.split(':').nth(1) {
            for group in groups.split(' ') {
                let grp = group.trim();
                if let Some(repo) = grp.strip_prefix(GROUP_PFX) {
                    result.push(repo.parse::<Repository>()?);
                }
            }
        }
        result.sort();
        Ok(result)
    }
    pub fn destroy(&self) -> DResult<()> {
        self.exists()?;
        sh(&format!(r#"userdel "{}""#, self.login()))?;
        let mut path = GIT_PATH.clone();
        path.push(format!(".config/cgit/{}.cgitrc", self.login()));
        let _r = fs::remove_file(path);
        println!(
            "User {}: {}",
            "destroyed".red().bold(),
            self.login_colored()
        );
        println!(
            "Remove user's home directory {} if not needed",
            self.home().to_string_lossy().blue().bold()
        );
        Ok(())
    }
    pub fn grant(&self, repo: &Repository) -> DResult<()> {
        self.exists()?;
        repo.exists()?;
        sh(&format!(
            r#"gpasswd -a "{}" "{}""#,
            self.login(),
            repo.group()
        ))?;
        self.chdir()?;
        if let Some(pos) = repo.name().rfind('/') {
            let base_dir = &repo.name()[..pos];
            fs::create_dir_all(base_dir)?;
            sh(&format!(
                r#"chown -R "{}" "{}""#,
                self.login(),
                &repo.name()[..repo.name().find('/').unwrap()]
            ))?;
            set_current_dir(base_dir)?;
        }
        let _r = fs::remove_file(repo.short_name());
        sh(&format!(
            r#"ln -sf "{}" "{}""#,
            repo.path_as_str(),
            repo.short_name()
        ))?;
        self.update_cgit()?;
        println!(
            "User {} has been {} access to {}",
            self.login_colored(),
            "granted".green().bold(),
            repo.name_colored()
        );
        Ok(())
    }
    pub fn print_all(short: bool) -> DResult<()> {
        let out = sh_any("grep /git-shell$ /etc/passwd")?;
        let mut result = Vec::new();
        for line in out.lines() {
            let mut sp = line.split(':');
            let login = sp.next().unwrap().parse::<User>()?;
            let name = sp.nth(3).unwrap().split(',').next().unwrap();
            result.push((login, name));
        }
        result.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
        for (user, name) in result {
            if short {
                println!("{}", user.login_colored());
            } else {
                println!("{} ({})", user.login_colored(), name);
            }
        }
        Ok(())
    }
    pub fn revoke(&self, repo: &Repository) -> DResult<()> {
        self.exists()?;
        sh_any(&format!(
            r#"gpasswd -d "{}" "{}""#,
            self.login(),
            repo.group()
        ))?;
        self.chdir()?;
        let _r = fs::remove_file(repo.name());
        let top_dir = repo.name().split('/').next().unwrap();
        sh_any(&format!(
            r#"find "{}" -type d -exec rmdir {{}} \; > /dev/null 2>&1"#,
            top_dir
        ))?;
        self.update_cgit()?;
        println!(
            "User {} has been {} access to {}",
            self.login_colored(),
            "revoked".red().bold(),
            repo.name_colored()
        );
        Ok(())
    }
    pub fn maintainer_set(&self, repo: &Repository) -> DResult<()> {
        self.exists()?;
        repo.set(&format!("hooks.user.{}.maintainer", self.login()), "true")?;
        println!(
            "User {} has been {} as maintainer in {}",
            self.login_colored(),
            "set".green().bold(),
            repo.name_colored()
        );
        Ok(())
    }
    pub fn maintainer_unset(&self, repo: &Repository) -> DResult<()> {
        repo.unset(&format!("hooks.user.{}.maintainer", self.login()))?;
        println!(
            "User {} has been {} as maintainer in {}",
            self.login_colored(),
            "unset".red().bold(),
            repo.name_colored()
        );
        Ok(())
    }
}
