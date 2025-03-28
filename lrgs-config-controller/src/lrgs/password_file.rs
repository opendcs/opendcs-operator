use sha1::{Digest, Sha1};
use std::{fs::File, io::Write};

pub struct DdsUser {
    pub username: String,
    pub password: String,
    pub roles: Vec<String>,
}

impl std::fmt::Display for DdsUser {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "({},{},{:?})", self.username, "*********", &self.roles)
    }
}

pub struct PasswordFile {
    users: Vec<DdsUser>,
}

fn to_line(user: &DdsUser) -> String {
    let pw_hash = lrgs_password_hash(&user.username, &user.password);
    let roles = if user.roles.is_empty() {
        String::from("none")
    } else {
        user.roles.join(",")
    };
    format!("{}:{roles}:{pw_hash}:", &user.username).to_string()
}

impl PasswordFile {
    pub fn new() -> PasswordFile {
        PasswordFile {
            users: vec![],
        }
    }

    pub fn add_user(&mut self, user: DdsUser) {
        self.users.push(user);
    }

    

    pub fn write_file(&self, mut f: &File) -> std::io::Result<()> {
        
        for user in self.users.as_slice() {
            writeln!(f,"{}", to_line(user));
        }
        Ok(())
    }

    pub fn to_string(&self) -> String {
        let mut buffer = String::new();
        for user in self.users.as_slice() {
            buffer.push_str(format!("{}\n",to_line(user)).as_str());
        }
        return buffer;
    }

}

impl std::fmt::Display for PasswordFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "PasswordFile(users = [")?;
        for user in self.users.as_slice() {
            write!(f, "{user},")?;
        }
        write!(f, "])")
    }
}

// to anyone thinking this isn't anywhere near sufficient,
// you are correct. This is temporary to adapt a legacy system.
fn lrgs_password_hash(username: &str, password: &str) -> String {
    let mut hasher = Sha1::new();
    hasher.update(username);
    hasher.update(password);
    hasher.update(username);
    hasher.update(password);
    let hash = hasher.finalize();
    base16ct::upper::encode_string(&hash)
}
