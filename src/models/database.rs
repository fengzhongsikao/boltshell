pub mod sqlite {

    use rusqlite::{Connection, Result, params};
    use serde::{Deserialize, Serialize};
    use std::sync::{Arc, Mutex};
    use tokio::task;

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct Session {
        pub id: i32,
        pub name: String,
        pub group_name: String,
        pub ip: String,
        pub port: String,
        pub user_name: String,
        pub password: String,
    }

    pub struct DatabaseManager {
        conn: Arc<Mutex<Connection>>,
    }

    impl DatabaseManager {
        pub fn new(db_path: &str) -> Result<Self> {
            let conn = Connection::open(db_path)?;
            // 创建用户表
            conn.execute(
                "CREATE TABLE IF NOT EXISTS sessions (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                group_name TEXT NOT NULL,
                ip TEXT UNIQUE NOT NULL,
                port TEXT NOT NULL,
                user_name TEXT UNIQUE NOT NULL,
                password TEXT NOT NULL,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP
            )",
                [],
            )?;

            Ok(Self {
                conn: Arc::new(Mutex::new(conn)),
            })
        }

        pub async fn add_session(
            &self,
            name: String,
            group_name: String,
            ip: String,
            port: String,
            user_name: String,
            password: String,
        ) -> Result<i32> {
            let conn = self.conn.clone();
            task::spawn_blocking(move || {
                let conn = conn.lock().unwrap();
                conn.execute(
                    "INSERT INTO sessions (name,group_name,ip,port,user_name,password) VALUES (?1,?2,?3,?4,?5,?6)",
                    params![name,group_name, ip,port,user_name,password],
                )?;
                Ok(conn.last_insert_rowid() as i32)
            })
            .await
            .unwrap()
        }

        pub async fn get_sessions(&self) -> Result<Vec<Session>> {
            let conn = self.conn.clone();
            task::spawn_blocking(move || {
                let conn = conn.lock().unwrap();
                let mut stmt = conn.prepare("SELECT id,name,group_name, ip,port,user_name,password,created_at FROM sessions")?;
                let user_iter = stmt.query_map([], |row| {
                    Ok(Session {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        group_name: row.get(2)?,
                        ip: row.get(3)?,
                        port: row.get(4)?,
                        user_name: row.get(5)?,
                        password: row.get(6)?,
                    })
                })?;

                let mut users = Vec::new();
                for user in user_iter {
                    users.push(user?);
                }
                Ok(users)
            })
            .await
            .unwrap()
        }

        pub async fn delete_session(&self, id: i32) -> Result<()> {
            let conn = self.conn.clone();

            task::spawn_blocking(move || {
                let conn = conn.lock().unwrap();
                conn.execute("DELETE FROM sessions WHERE id = ?1", params![id])?;
                Ok(())
            })
            .await
            .unwrap()
        }

        pub async fn update_session(
            &self,
            id: i32,
            name: String,
            group_name: String,
            ip: String,
            port: String,
            user_name: String,
            password: String,
        ) -> Result<()> {
            let conn = self.conn.clone();
            task::spawn_blocking(move || {
                let conn = conn.lock().unwrap();
                conn.execute(
                    "UPDATE sessions
             SET name = ?1,
                 group_name = ?2,
                 ip = ?3,
                 port = ?4,
                 user_name = ?5,
                 password = ?6
             WHERE id = ?7",
                    params![name, group_name, ip, port, user_name, password, id],
                )?;
                Ok(())
            })
            .await
            .unwrap()
        }
    }
}
