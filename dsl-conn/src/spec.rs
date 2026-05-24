//! ConnectionSpec: the shape the LSP receives from `initializationOptions`.
//!
//! Mirrors what the editor's `db_manager` plugin stores on disk so there
//! is no field translation between client and server.

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct ConnectionSpec {
    pub name: String,
    pub driver: String,
    #[serde(default)]
    pub host: Option<String>,
    #[serde(default)]
    pub port: Option<u16>,
    #[serde(default)]
    pub user: Option<String>,
    #[serde(default)]
    pub password: Option<String>,
    #[serde(default)]
    pub database: Option<String>,
    #[serde(default)]
    pub schema: Option<String>,
}

impl ConnectionSpec {
    /// Build the URL form sqlx accepts. Password is included.
    pub fn url(&self) -> String {
        match self.driver.as_str() {
            "postgres" | "postgresql" => {
                let user = self.user.clone().unwrap_or_default();
                let pass = self.password.clone().unwrap_or_default();
                let host = self.host.clone().unwrap_or_else(|| "localhost".into());
                let port = self.port.unwrap_or(5432);
                let db = self.database.clone().unwrap_or_default();
                if pass.is_empty() {
                    format!("postgres://{user}@{host}:{port}/{db}")
                } else {
                    format!("postgres://{user}:{pass}@{host}:{port}/{db}")
                }
            }
            "mysql" | "mariadb" => {
                let user = self.user.clone().unwrap_or_default();
                let pass = self.password.clone().unwrap_or_default();
                let host = self.host.clone().unwrap_or_else(|| "localhost".into());
                let port = self.port.unwrap_or(3306);
                let db = self.database.clone().unwrap_or_default();
                if pass.is_empty() {
                    format!("mysql://{user}@{host}:{port}/{db}")
                } else {
                    format!("mysql://{user}:{pass}@{host}:{port}/{db}")
                }
            }
            "sqlite" | "sqlite3" => {
                // `database` carries the file path. Empty → in-memory.
                let path = self.database.clone().unwrap_or_default();
                if path.is_empty() {
                    "sqlite::memory:".into()
                } else {
                    format!("sqlite://{path}")
                }
            }
            _ => String::new(),
        }
    }
}
