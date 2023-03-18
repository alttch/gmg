use crate::user::User;
use crate::{
    chmod, err, failed, sh, sh_any, DResult, ErrorKind, GIT_PATH, GIT_USER, GROUP_PFX, MAIN_BRANCH,
    PROTECTED_BRANCHES, VERSION,
};
use bmart_derive::Sorting;
use colored::{ColoredString, Colorize};
use configparser::ini::Ini;
use std::env::set_current_dir;
use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use tempdir::TempDir;

#[derive(Clone, Sorting)]
#[sorting(id = "name")]
pub struct Repository {
    name: String,
    group: String,
    path: PathBuf,
}

impl FromStr for Repository {
    type Err = ErrorKind;
    fn from_str(name: &str) -> Result<Self, Self::Err> {
        if name.starts_with('/') {
            return Err(ErrorKind::Failed(
                "repository name can not start with /".to_owned(),
            ));
        }
        #[allow(clippy::case_sensitive_file_extension_comparisons)]
        if name.ends_with(".git") || name.contains(".git/") {
            return Err(ErrorKind::Failed(
                "repository name can not end with or contain .git in path chunks".to_owned(),
            ));
        }
        if name.len() > 30 {
            return Err(ErrorKind::Failed(
                "repository name is longer than 30 chars".to_owned(),
            ));
        }
        let group = format!("{}{}", GROUP_PFX, name);
        let mut path = GIT_PATH.clone();
        path.push(format!("{}.git", name));
        Ok(Self {
            name: name.to_owned(),
            group,
            path,
        })
    }
}

impl Repository {
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn name_colored(&self) -> ColoredString {
        self.name.cyan().bold()
    }
    pub fn read_description(&self) -> DResult<Option<String>> {
        let mut path = self.path.clone();
        path.push("description");
        let desc = fs::read_to_string(path)?;
        if desc.starts_with("Unnamed repository;") {
            Ok(None)
        } else {
            Ok(Some(desc.trim().to_string()))
        }
    }
    pub fn set_description(&self, desc: Option<&str>) -> DResult<()> {
        self.exists()?;
        let mut path = self.path.clone();
        path.push("description");
        fs::write(path, desc.unwrap_or("Unnamed repository;"))?;
        for user in self.users()? {
            user.update_cgit()?;
        }
        Ok(())
    }
    /// # Panics
    ///
    /// should not panic
    pub fn short_name(&self) -> &str {
        self.name.rsplit('/').next().unwrap()
    }
    pub fn group(&self) -> &str {
        &self.group
    }
    pub fn path(&self) -> &Path {
        &self.path
    }
    pub fn path_as_str(&self) -> std::borrow::Cow<str> {
        self.path.to_string_lossy()
    }
    pub fn chdir(&self) -> DResult<()> {
        set_current_dir(&self.path)?;
        Ok(())
    }
    pub fn load_config(&self) -> DResult<Ini> {
        let mut path = self.path.clone();
        path.push("config");
        let mut config = Ini::new();
        config.load(path)?;
        Ok(config)
    }
    pub fn exists(&self) -> DResult<()> {
        if self.path.exists() {
            Ok(())
        } else {
            failed!(format!("Repository doesn't exist: {}", self.name))
        }
    }
    #[inline]
    pub fn protect(&self, branch: &str) -> DResult<()> {
        self.set(&format!("hooks.branch.{}.protected", branch), "true")
    }
    #[inline]
    pub fn unprotect(&self, branch: &str) -> DResult<()> {
        self.unset(&format!("hooks.branch.{}.protected", branch))
    }
    pub fn set(&self, param: &str, value: &str) -> DResult<()> {
        self.exists()?;
        let mut config_path = self.path.clone();
        config_path.push("config");
        sh(&format!(
            r#"git config -f "{}" {} "{}""#,
            config_path.to_string_lossy(),
            param,
            value
        ))?;
        chmod(&config_path, 0o644)?;
        Ok(())
    }
    pub fn unset(&self, param: &str) -> DResult<()> {
        self.exists()?;
        let mut config_path = self.path.clone();
        config_path.push("config");
        sh(&format!(
            r#"git config -f "{}" --unset {}"#,
            config_path.to_string_lossy(),
            param,
        ))?;
        chmod(&config_path, 0o644)?;
        Ok(())
    }
    pub fn archive(&self) -> DResult<()> {
        self.exists()?;
        sh(&format!("groupdel {}", self.group()))?;
        chmod(self.path(), 0o700)?;
        println!("Repository archived: {}", self.name_colored());
        Ok(())
    }
    pub fn branches(&self) -> DResult<Vec<String>> {
        self.exists()?;
        self.chdir()?;
        let out = sh("git branch")?;
        let mut result = Vec::new();
        for line in out.lines() {
            let name = line.strip_prefix('*').unwrap_or(line);
            result.push(name.trim().to_owned());
        }
        result.sort();
        Ok(result)
    }
    pub fn check(&self) -> DResult<()> {
        self.exists()?;
        self.chdir()?;
        if let Err(e) = sh("git fsck") {
            err!(format!(
                "Repository {} failed to check\n{}\n",
                self.name(),
                e
            ));
        }
        Ok(())
    }
    pub fn cleanup(&self) -> DResult<()> {
        self.exists()?;
        self.chdir()?;
        sh("git reflog expire --expire=now --all")?;
        sh("git gc --prune=now")?;
        self.fix(false)?;
        Ok(())
    }
    pub fn fix(&self, full: bool) -> DResult<()> {
        sh(&format!(
            r#"find {} -type d -exec chmod -R 002775 {{}} \;"#,
            self.path_as_str()
        ))?;
        sh(&format!(
            r#"find {} -type f -exec chmod -R 000664 {{}} \;"#,
            self.path_as_str()
        ))?;
        chmod(self.path(), 0o2770)?;
        sh(&format!(
            r#"chmod -R 000755 "{}/hooks""#,
            self.path_as_str()
        ))?;
        sh(&format!(
            r#"chown -R "{}:{}" "{}""#,
            GIT_USER,
            self.group(),
            self.path_as_str()
        ))?;
        sh(&format!(
            r#"chown "root:{}" "{}/config""#,
            GIT_USER,
            self.path_as_str()
        ))?;
        sh(&format!(
            r#"chown "root:{}" "{}/description""#,
            GIT_USER,
            self.path_as_str()
        ))?;
        let mut path = self.path.clone();
        path.push("config");
        chmod(path, 0o000_644)?;
        let mut path = self.path.clone();
        path.push("description");
        chmod(path, 0o000_644)?;
        if full {
            self.cleanup()?;
        }
        Ok(())
    }
    pub fn create(&self, init_only: bool, description: Option<&str>) -> DResult<()> {
        if self.exists().is_ok() {
            return failed!("repository already exists".to_owned());
        }
        fs::create_dir_all(self.path())?;
        sh(&format!("groupadd {}", self.group()))?;
        set_current_dir(&*GIT_PATH)?;
        sh(&format!(
            r#"git init -q -b "{}" --bare --shared=group "{}.git""#,
            MAIN_BRANCH,
            self.name()
        ))?;
        self.fix(false)?;
        self.set("gmg.version", VERSION)?;
        self.set("receive.denyNonFastForwards", "false")?;
        if init_only {
            println!("Repository initialized: {}", self.name_colored());
            return Ok(());
        }
        self.do_initial_commit()?;
        self.set_description(description)?;
        for branch in PROTECTED_BRANCHES {
            self.protect(branch)?;
        }
        println!("Repository created: {}", self.name_colored());
        Ok(())
    }
    fn do_initial_commit(&self) -> DResult<()> {
        let dir = TempDir::new("gmg")?;
        set_current_dir(dir.path())?;
        sh(&format!(
            r#"git clone --quiet "{}" > /dev/null 2>&1"#,
            self.path_as_str()
        ))?;
        let short_name = self.short_name();
        set_current_dir(short_name)?;
        fs::write("README.md", format!("# {}", short_name))?;
        sh("git add README.md")?;
        sh("git commit --quiet -a -m init")?;
        sh(&format!(r#"git push --quiet origin "{}""#, MAIN_BRANCH))?;
        dir.close()?;
        self.chdir()?;
        Ok(())
    }
    pub fn destroy(&self) -> DResult<()> {
        self.exists()?;
        for user in self.users()? {
            user.revoke(self)?;
        }
        sh(&format!(r#"groupdel "{}""#, self.group()))?;
        fs::remove_dir_all(self.path())?;
        set_current_dir(&*GIT_PATH)?;
        let mut sp = self.name.split('/');
        let top_dir = sp.next().unwrap();
        if sp.next().is_some() {
            sh_any(&format!(
                r#"find "{}" -type d -not -path "*/*.git/*" -not -path "*/*.git" -exec rmdir {{}} \; > /dev/null 2>&1"#,
                top_dir
            ))?;
            let _r = fs::remove_dir(top_dir);
        }
        println!(
            "Repository {}: {}",
            "destroyed".red().bold(),
            self.name_colored()
        );
        Ok(())
    }
    pub fn rename(&self, new_repo: &Repository) -> DResult<()> {
        self.exists()?;
        new_repo.create(true, None)?;
        match self.replace_and_move(new_repo) {
            Ok(()) => self.destroy(),
            Err(e) => {
                if let Err(err_des) = new_repo.destroy() {
                    err!(err_des.to_string());
                }
                Err(e)
            }
        }
    }
    fn replace_and_move(&self, target: &Repository) -> DResult<()> {
        if target.path_as_str().len() + 2 < GIT_PATH.to_string_lossy().len() {
            return failed!("invalid repo path".to_owned());
        }
        sh(&format!("rm -rf {}/*", target.path_as_str()))?;
        sh(&format!(
            "cp -prf {}/* {}/",
            self.path_as_str(),
            target.path_as_str()
        ))?;
        target.fix(false)?;
        for user in self.users()? {
            user.grant(target)?;
        }
        Ok(())
    }
    pub fn print_all(short: bool) -> DResult<()> {
        set_current_dir(&*GIT_PATH)?;
        let out = sh(r#"find . -name "*.git" -type d"#)?;
        let mut result = Vec::new();
        for line in out.lines() {
            if let Some(l) = line.strip_prefix("./") {
                if let Some(n) = l.strip_suffix(".git") {
                    result.push(n);
                }
            }
        }
        result.sort_unstable();
        for r in result {
            let repo = r.parse::<Repository>()?;
            if short {
                println!("{}", repo.name_colored());
            } else {
                println!(
                    "{} ({})",
                    repo.name_colored(),
                    repo.read_description()?.unwrap_or_default()
                );
            }
        }
        Ok(())
    }
    pub fn users(&self) -> DResult<Vec<User>> {
        self.exists()?;
        let out = sh_any(&format!(r#"grep "^{}:" /etc/group"#, self.group()))?;
        let mut users = Vec::new();
        for line in out.lines() {
            for user in line.split(':').nth(3).unwrap().split(',') {
                if !user.is_empty() {
                    users.push(user.parse()?);
                }
            }
        }
        users.sort();
        Ok(users)
    }
    pub fn print_info(&self) -> DResult<()> {
        self.exists()?;
        self.chdir()?;
        let config = self.load_config()?;
        let mut protected_branches = Vec::new();
        let mut maintainers = Vec::new();
        for section in config.sections() {
            if let Some(hooks_section) = section.strip_prefix("hooks") {
                let name = hooks_section.trim();
                if name.len() > 2 {
                    let sn = &name[1..name.len() - 1];
                    let mut sp = sn.splitn(2, '.');
                    match sp.next().unwrap() {
                        "branch" => {
                            if let Some(branch) = sp.next() {
                                if let Some(p) = config.get(&section, "protected") {
                                    if p == "true" {
                                        protected_branches.push(branch.to_owned());
                                    }
                                }
                            }
                        }
                        "user" => {
                            if let Some(login) = sp.next() {
                                if let Some(p) = config.get(&section, "maintainer") {
                                    if p == "true" {
                                        maintainers.push(login.to_owned());
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
        protected_branches.sort();
        maintainers.sort();
        println!("name: {}", self.name_colored());
        if let Some(desc) = self.read_description()? {
            println!("description: {}", desc);
        }
        println!("path: {}", self.path_as_str().white());
        println!("branches:");
        for r in self.branches()? {
            println!(" {}", r.yellow());
        }
        println!("protected branches:");
        for r in protected_branches {
            println!(" {}", r.green());
        }
        println!("users:");
        for u in self.users()? {
            println!(" {}", u.login_colored());
        }
        println!("maintainers:");
        for r in maintainers {
            println!(" {}", r.green());
        }
        Ok(())
    }
}
