use std::{fs, path::Path};

use rusqlite::{Connection, OptionalExtension, params};

use crate::models::{NewProvider, ProviderConfig, UsageSnapshot};

pub struct Database {
    path: std::path::PathBuf,
}

impl Database {
    pub fn open(path: &Path) -> Result<Self, rusqlite::Error> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|_| rusqlite::Error::InvalidPath(path.to_path_buf()))?;
        }
        let db = Self {
            path: path.to_path_buf(),
        };
        db.connection()?
            .execute_batch("PRAGMA foreign_keys = ON; PRAGMA journal_mode = WAL;")?;
        Ok(db)
    }

    fn connection(&self) -> Result<Connection, rusqlite::Error> {
        Connection::open(&self.path)
    }

    pub fn migrate(&self) -> Result<(), rusqlite::Error> {
        self.connection()?
            .execute_batch(include_str!("../migrations/001_initial.sql"))
    }

    pub fn list_providers(&self) -> Result<Vec<ProviderConfig>, rusqlite::Error> {
        let connection = self.connection()?;
        let mut statement = connection.prepare("SELECT id, provider_type, display_name, secret_ref, enabled FROM providers ORDER BY created_at DESC")?;
        statement
            .query_map([], |row| {
                Ok(ProviderConfig {
                    id: row.get(0)?,
                    provider_type: row.get(1)?,
                    display_name: row.get(2)?,
                    secret_ref: row.get(3)?,
                    enabled: row.get::<_, i64>(4)? != 0,
                })
            })?
            .collect()
    }

    pub fn find_provider(&self, id: &str) -> Result<Option<ProviderConfig>, rusqlite::Error> {
        self.connection()?.query_row("SELECT id, provider_type, display_name, secret_ref, enabled FROM providers WHERE id = ?1", [id], |row| Ok(ProviderConfig { id: row.get(0)?, provider_type: row.get(1)?, display_name: row.get(2)?, secret_ref: row.get(3)?, enabled: row.get::<_, i64>(4)? != 0 })).optional()
    }

    pub fn add_provider(&self, input: NewProvider) -> Result<(), rusqlite::Error> {
        let now = now();
        let id = format!(
            "{}-{}",
            input.provider_type,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis()
        );
        self.connection()?.execute("INSERT INTO providers (id, provider_type, display_name, secret_ref, enabled, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, 1, ?5, ?5)", params![id, input.provider_type, input.display_name, input.secret_ref, now])?;
        Ok(())
    }

    pub fn latest_snapshot(
        &self,
        provider_id: &str,
    ) -> Result<Option<UsageSnapshot>, rusqlite::Error> {
        self.connection()?.query_row("SELECT timestamp, status, cost, currency, raw_metrics_json FROM usage_snapshots WHERE provider_id = ?1 ORDER BY id DESC LIMIT 1", [provider_id], |row| Ok(UsageSnapshot { provider_id: provider_id.to_owned(), timestamp: row.get(0)?, status: row.get(1)?, cost: row.get(2)?, currency: row.get(3)?, metrics: serde_json::from_str(&row.get::<_, String>(4)?).unwrap_or_default() })).optional()
    }

    pub fn save_snapshot(&self, snapshot: &UsageSnapshot) -> Result<(), rusqlite::Error> {
        let metrics = serde_json::to_string(&snapshot.metrics).unwrap_or_else(|_| "[]".to_owned());
        self.connection()?.execute("INSERT INTO usage_snapshots (provider_id, timestamp, status, cost, currency, raw_metrics_json) VALUES (?1, ?2, ?3, ?4, ?5, ?6)", params![snapshot.provider_id, snapshot.timestamp, snapshot.status, snapshot.cost, snapshot.currency, metrics])?;
        Ok(())
    }
}

fn now() -> String {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        .to_string()
}
