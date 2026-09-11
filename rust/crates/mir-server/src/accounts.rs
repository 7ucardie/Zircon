//! Account and character persistence: a JSON file with Argon2 password hashes.
//!
//! Writes are atomic (temp file + rename). The store is small and fully in
//! memory; it is saved on every account-level change and periodically.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use argon2::password_hash::{
    rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString,
};
use argon2::Argon2;
use mir_proto::{rules, CharacterSummary, Class, Direction, Gender, Point};
use serde::{Deserialize, Serialize};

use crate::items::StoredItem;
use crate::magic::StoredMagic;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CharacterRecord {
    pub id: u32,
    pub name: String,
    pub class: Class,
    pub gender: Gender,
    pub hair: u8,
    pub level: i32,
    pub experience: u64,
    pub hp: i32,
    pub mp: i32,
    /// Map file stem; empty until the character has entered the world.
    pub map: String,
    pub location: Point,
    pub direction: Direction,
    pub created: u64,
    pub last_login: u64,
    pub deleted: bool,
    #[serde(default)]
    pub items: Vec<StoredItem>,
    #[serde(default)]
    pub gold: u64,
    /// Highest item id handed out for this character.
    #[serde(default)]
    pub next_item_id: u32,
    #[serde(default)]
    pub magics: Vec<StoredMagic>,
}

impl CharacterRecord {
    pub fn summary(&self) -> CharacterSummary {
        CharacterSummary {
            id: self.id,
            name: self.name.clone(),
            class: self.class,
            gender: self.gender,
            hair: self.hair,
            level: self.level.clamp(1, 255) as u8,
            last_login: self.last_login,
            location: String::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountRecord {
    pub id: u32,
    pub email: String,
    pub password_hash: String,
    pub created: u64,
    pub last_login: u64,
    pub characters: Vec<CharacterRecord>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct Store {
    next_account_id: u32,
    next_character_id: u32,
    accounts: Vec<AccountRecord>,
}

pub struct Accounts {
    path: PathBuf,
    store: Store,
    dirty: bool,
    /// lower-cased email -> index into `store.accounts`
    by_email: HashMap<String, usize>,
}

pub fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

impl Accounts {
    pub fn load(path: impl AsRef<Path>) -> Result<Accounts> {
        let path = path.as_ref().to_path_buf();
        let store = if path.exists() {
            let text = std::fs::read_to_string(&path)
                .with_context(|| format!("reading {}", path.display()))?;
            serde_json::from_str(&text).with_context(|| format!("parsing {}", path.display()))?
        } else {
            Store {
                next_account_id: 1,
                next_character_id: 1,
                accounts: Vec::new(),
            }
        };
        let by_email = store
            .accounts
            .iter()
            .enumerate()
            .map(|(i, a)| (a.email.to_lowercase(), i))
            .collect();
        tracing::info!(path = %path.display(), accounts = store.accounts.len(), "accounts loaded");
        Ok(Accounts {
            path,
            store,
            dirty: false,
            by_email,
        })
    }

    pub fn save_if_dirty(&mut self) -> Result<()> {
        if !self.dirty {
            return Ok(());
        }
        if let Some(dir) = self.path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let tmp = self.path.with_extension("json.tmp");
        let text = serde_json::to_string_pretty(&self.store)?;
        std::fs::write(&tmp, text)?;
        std::fs::rename(&tmp, &self.path)?;
        self.dirty = false;
        tracing::debug!(path = %self.path.display(), "accounts saved");
        Ok(())
    }

    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    pub fn create(&mut self, email: &str, password: &str) -> std::result::Result<u32, String> {
        let email = email.trim();
        if !rules::valid_email(email) {
            return Err("Invalid account name".into());
        }
        if !rules::valid_password(password) {
            return Err(format!(
                "Password must be {}-{} characters",
                rules::PASSWORD_MIN,
                rules::PASSWORD_MAX
            ));
        }
        if self.by_email.contains_key(&email.to_lowercase()) {
            return Err("Account already exists".into());
        }
        let salt = SaltString::generate(&mut OsRng);
        let hash = Argon2::default()
            .hash_password(password.as_bytes(), &salt)
            .map_err(|e| format!("hashing failed: {e}"))?
            .to_string();
        let id = self.store.next_account_id;
        self.store.next_account_id += 1;
        self.store.accounts.push(AccountRecord {
            id,
            email: email.to_string(),
            password_hash: hash,
            created: now_secs(),
            last_login: 0,
            characters: Vec::new(),
        });
        self.by_email
            .insert(email.to_lowercase(), self.store.accounts.len() - 1);
        self.dirty = true;
        Ok(id)
    }

    /// Verify credentials and return the account id.
    pub fn login(&mut self, email: &str, password: &str) -> std::result::Result<u32, String> {
        let idx = *self
            .by_email
            .get(&email.trim().to_lowercase())
            .ok_or_else(|| "Account not found".to_string())?;
        let account = &mut self.store.accounts[idx];
        let parsed = PasswordHash::new(&account.password_hash).map_err(|e| e.to_string())?;
        if Argon2::default()
            .verify_password(password.as_bytes(), &parsed)
            .is_err()
        {
            return Err("Wrong password".into());
        }
        account.last_login = now_secs();
        self.dirty = true;
        Ok(account.id)
    }

    pub fn account(&self, id: u32) -> Option<&AccountRecord> {
        self.store.accounts.iter().find(|a| a.id == id)
    }

    pub fn account_mut(&mut self, id: u32) -> Option<&mut AccountRecord> {
        self.store.accounts.iter_mut().find(|a| a.id == id)
    }

    pub fn summaries(&self, account: u32) -> Vec<CharacterSummary> {
        self.account(account)
            .map(|a| {
                a.characters
                    .iter()
                    .filter(|c| !c.deleted)
                    .map(|c| c.summary())
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn new_character(
        &mut self,
        account: u32,
        name: &str,
        class: Class,
        gender: Gender,
        hair: u8,
    ) -> std::result::Result<CharacterRecord, String> {
        let name = name.trim();
        if !rules::valid_name(name) {
            return Err(format!(
                "Name must be {}-{} letters or digits and start with a letter",
                rules::NAME_MIN,
                rules::NAME_MAX
            ));
        }
        if hair == 0 || hair > rules::HAIR_TYPES {
            return Err("Invalid hair".into());
        }
        let taken = self.store.accounts.iter().any(|a| {
            a.characters
                .iter()
                .any(|c| !c.deleted && c.name.eq_ignore_ascii_case(name))
        });
        if taken {
            return Err("Name already taken".into());
        }
        let id = self.store.next_character_id;
        let acc = self
            .account_mut(account)
            .ok_or_else(|| "No such account".to_string())?;
        if acc.characters.iter().filter(|c| !c.deleted).count() >= rules::MAX_CHARACTERS {
            return Err(format!("Maximum {} characters", rules::MAX_CHARACTERS));
        }
        let rec = CharacterRecord {
            id,
            name: name.to_string(),
            class,
            gender,
            hair,
            level: 1,
            experience: 0,
            hp: 0,
            mp: 0,
            map: String::new(),
            location: Point::default(),
            direction: Direction::Down,
            created: now_secs(),
            last_login: 0,
            deleted: false,
            items: Vec::new(),
            gold: 0,
            next_item_id: 0,
            magics: Vec::new(),
        };
        acc.characters.push(rec.clone());
        self.store.next_character_id += 1;
        self.dirty = true;
        Ok(rec)
    }

    pub fn delete_character(&mut self, account: u32, id: u32) -> std::result::Result<(), String> {
        let acc = self
            .account_mut(account)
            .ok_or_else(|| "No such account".to_string())?;
        let c = acc
            .characters
            .iter_mut()
            .find(|c| c.id == id && !c.deleted)
            .ok_or_else(|| "No such character".to_string())?;
        c.deleted = true;
        self.dirty = true;
        Ok(())
    }

    pub fn character(&self, account: u32, id: u32) -> Option<&CharacterRecord> {
        self.account(account)?
            .characters
            .iter()
            .find(|c| c.id == id && !c.deleted)
    }

    pub fn character_mut(&mut self, account: u32, id: u32) -> Option<&mut CharacterRecord> {
        self.account_mut(account)?
            .characters
            .iter_mut()
            .find(|c| c.id == id && !c.deleted)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn account_lifecycle() {
        let dir = std::env::temp_dir().join(format!("zircon-accounts-{}", std::process::id()));
        let path = dir.join("accounts.json");
        let mut a = Accounts::load(&path).unwrap();
        assert!(a.create("bob", "short").is_err());
        let id = a.create("bob@example.com", "secret123").unwrap();
        assert!(a.create("BOB@example.com", "secret123").is_err());
        assert!(a.login("bob@example.com", "wrong").is_err());
        assert_eq!(a.login("Bob@example.com", "secret123").unwrap(), id);
        let c = a
            .new_character(id, "Hero", Class::Warrior, Gender::Male, 1)
            .unwrap();
        assert!(a
            .new_character(id, "hero", Class::Wizard, Gender::Female, 2)
            .is_err());
        assert!(a
            .new_character(id, "1bad", Class::Wizard, Gender::Female, 2)
            .is_err());
        assert_eq!(a.summaries(id).len(), 1);
        a.save_if_dirty().unwrap();
        let b = Accounts::load(&path).unwrap();
        assert_eq!(b.summaries(id)[0].name, "Hero");
        let mut b = b;
        b.delete_character(id, c.id).unwrap();
        assert!(b.summaries(id).is_empty());
        let _ = std::fs::remove_dir_all(dir);
    }
}
