use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::io::AsyncWriteExt;
use tokio::sync::Mutex;

/// A single chat message stored per-contact.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ChatMessage {
    pub id: u64,
    pub contact_address: String,
    pub content: String,
    pub is_sent: bool,
    pub status: String,
    pub timestamp: i64,
}

/// Summary of a conversation (latest message details).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ConversationSummary {
    pub contact_address: String,
    pub last_content: String,
    pub last_timestamp: i64,
    pub last_is_sent: bool,
    pub unread_count: u64,
}

/// Local profile info attached to an address. Never shared over network.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct UserProfile {
    /// Custom display name / nickname.
    pub name: String,
    /// Arbitrary notes about this contact.
    pub notes: String,
    /// Relative path (within the user dir) to the avatar image, e.g. "images/avatar.jpg".
    pub avatar_path: Option<String>,
    /// Unix timestamp when this contact was first added locally.
    pub added_at: i64,

    // ---- Extended personal info (all local-only) ----
    pub nickname: String,
    pub gender: String,
    pub age: u32,
    pub country: String,
    pub ethnicity: String,
    pub blood_type: String,
    pub height_cm: u32,
    pub weight_kg: u32,
    pub education: String,
    pub home_address: String,
}

impl Default for UserProfile {
    fn default() -> Self {
        Self {
            name: String::new(),
            notes: String::new(),
            avatar_path: None,
            added_at: chrono::Utc::now().timestamp(),
            nickname: String::new(),
            gender: String::new(),
            age: 0,
            country: String::new(),
            ethnicity: String::new(),
            blood_type: String::new(),
            height_cm: 0,
            weight_kg: 0,
            education: String::new(),
            home_address: String::new(),
        }
    }
}

/// File-based user data store. Each address gets its own directory
/// under `<base_path>/users/<address>/`.
///
/// Directory layout:
/// ```text
/// <base_path>/users/
///   <address>/
///     chat.json        # Chat messages
///     profile.json     # Local profile / personal info
///     images/          # Image files
///     <any other file> # Extensible via store_file / read_file
/// ```
///
/// All data is local-only and never shared over the network.
pub struct UserStore {
    base_path: PathBuf,
    lock: tokio::sync::Mutex<()>,
    profile_cache: Arc<Mutex<HashMap<String, UserProfile>>>,
}

impl UserStore {
    pub fn new(base_path: PathBuf) -> Self {
        Self {
            base_path,
            lock: tokio::sync::Mutex::new(()),
            profile_cache: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn clear_profile_cache(&self) {
        let cache = self.profile_cache.clone();
        tokio::spawn(async move {
            cache.lock().await.clear();
        });
    }

    fn user_dir(&self, address: &str) -> PathBuf {
        self.base_path.join("users").join(address)
    }

    fn images_dir(&self, address: &str) -> PathBuf {
        self.user_dir(address).join("images")
    }

    fn chat_path(&self, address: &str) -> PathBuf {
        self.user_dir(address).join("chat.json")
    }

    async fn ensure_user_dir(&self, address: &str) -> std::io::Result<PathBuf> {
        let dir = self.user_dir(address);
        tokio::fs::create_dir_all(&dir).await?;
        Ok(dir)
    }

    async fn ensure_images_dir(&self, address: &str) -> std::io::Result<PathBuf> {
        let dir = self.images_dir(address);
        tokio::fs::create_dir_all(&dir).await?;
        Ok(dir)
    }

    // ---------------------------------------------------------------
    //  Generic file helpers
    // ---------------------------------------------------------------

    /// Write an arbitrary file inside the user's directory.
    /// Creates the user directory if it doesn't exist. Subdirectories
    /// (e.g. "images/foo.jpg") are supported.
    pub async fn store_file(
        &self,
        address: &str,
        rel_path: &str,
        data: &[u8],
    ) -> anyhow::Result<()> {
        let _guard = self.lock.lock().await;
        let full = self.user_dir(address).join(rel_path);
        if let Some(parent) = full.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        let mut f = tokio::fs::File::create(&full).await?;
        f.write_all(data).await?;
        f.flush().await?;
        Ok(())
    }

    /// Read an arbitrary file from inside the user's directory.
    /// Returns `None` if the file does not exist.
    pub async fn read_file(
        &self,
        address: &str,
        rel_path: &str,
    ) -> anyhow::Result<Option<Vec<u8>>> {
        let full = self.user_dir(address).join(rel_path);
        match tokio::fs::read(&full).await {
            Ok(data) => Ok(Some(data)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Delete a file inside the user's directory. Returns whether the file existed.
    pub async fn delete_file(&self, address: &str, rel_path: &str) -> anyhow::Result<bool> {
        let _guard = self.lock.lock().await;
        let full = self.user_dir(address).join(rel_path);
        match tokio::fs::remove_file(&full).await {
            Ok(()) => Ok(true),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(e) => Err(e.into()),
        }
    }

    /// List all files (non-recursive) inside a subdirectory of the user's directory.
    pub async fn list_files(&self, address: &str, subdir: &str) -> anyhow::Result<Vec<String>> {
        let full = self.user_dir(address).join(subdir);
        let mut names = Vec::new();
        let mut entries = match tokio::fs::read_dir(&full).await {
            Ok(e) => e,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(names),
            Err(e) => return Err(e.into()),
        };
        while let Some(entry) = entries.next_entry().await? {
            if entry.file_type().await?.is_file() {
                if let Some(name) = entry.file_name().to_str() {
                    names.push(name.to_string());
                }
            }
        }
        names.sort();
        Ok(names)
    }

    /// Save a JSON-serializable value to a file inside the user's directory.
    pub async fn store_json<T: serde::Serialize>(
        &self,
        address: &str,
        rel_path: &str,
        value: &T,
    ) -> anyhow::Result<()> {
        let data = serde_json::to_string_pretty(value)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        self.store_file(address, rel_path, data.as_bytes()).await
    }

    /// Load and deserialize a JSON value from a file inside the user's directory.
    /// Returns `None` if the file does not exist.
    pub async fn load_json<T: serde::de::DeserializeOwned>(
        &self,
        address: &str,
        rel_path: &str,
    ) -> anyhow::Result<Option<T>> {
        let bytes = self.read_file(address, rel_path).await?;
        match bytes {
            Some(data) => {
                let value = serde_json::from_slice(&data)
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
                Ok(Some(value))
            }
            None => Ok(None),
        }
    }

    // ---------------------------------------------------------------
    //  Chat messages
    // ---------------------------------------------------------------

    async fn read_messages_raw(&self, address: &str) -> std::io::Result<Vec<ChatMessage>> {
        let path = self.chat_path(address);
        match tokio::fs::read_to_string(&path).await {
            Ok(content) => {
                if content.trim().is_empty() {
                    return Ok(Vec::new());
                }
                serde_json::from_str(&content)
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
            Err(e) => Err(e),
        }
    }

    async fn write_messages_raw(&self, address: &str, msgs: &[ChatMessage]) -> std::io::Result<()> {
        let content = serde_json::to_string_pretty(msgs)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        // Caller already holds self.lock – write directly without re-locking.
        let full = self.user_dir(address).join("chat.json");
        if let Some(parent) = full.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        }
        tokio::fs::write(&full, content.as_bytes())
            .await
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        Ok(())
    }

    /// Add a new chat message for a contact. Returns the saved message with assigned id.
    pub async fn add_message(
        &self,
        contact: &str,
        content: &str,
        is_sent: bool,
        status: &str,
    ) -> anyhow::Result<ChatMessage> {
        let _guard = self.lock.lock().await;
        self.ensure_user_dir(contact).await?;
        let mut msgs = self.read_messages_raw(contact).await?;
        let next_id = msgs.last().map(|m| m.id + 1).unwrap_or(1);
        let msg = ChatMessage {
            id: next_id,
            contact_address: contact.to_string(),
            content: content.to_string(),
            is_sent,
            status: status.to_string(),
            timestamp: chrono::Utc::now().timestamp(),
        };
        msgs.push(msg.clone());
        self.write_messages_raw(contact, &msgs).await?;
        Ok(msg)
    }

    /// Update the status of the last sent message to a contact.
    pub async fn update_last_sent_status(
        &self,
        contact: &str,
        new_status: &str,
    ) -> anyhow::Result<()> {
        let _guard = self.lock.lock().await;
        let mut msgs = self.read_messages_raw(contact).await?;
        let mut changed = false;
        if let Some(last) = msgs.iter_mut().rev().find(|m| m.is_sent) {
            last.status = new_status.to_string();
            changed = true;
        }
        if changed {
            self.write_messages_raw(contact, &msgs).await?;
        }
        Ok(())
    }

    /// Get all messages for a contact, ordered by id ascending.
    pub async fn get_messages(&self, contact: &str) -> anyhow::Result<Vec<ChatMessage>> {
        let msgs = self.read_messages_raw(contact).await?;
        Ok(msgs)
    }

    /// Mark all *received* messages as "read".
    pub async fn mark_as_read(&self, contact: &str) -> anyhow::Result<()> {
        let _guard = self.lock.lock().await;
        let mut msgs = self.read_messages_raw(contact).await?;
        let mut changed = false;
        for m in &mut msgs {
            if !m.is_sent && m.status == "received" {
                m.status = "read".to_string();
                changed = true;
            }
        }
        if changed {
            self.write_messages_raw(contact, &msgs).await?;
        }
        Ok(())
    }

    /// Count unread (received but not yet read) messages for a contact.
    pub async fn get_unread_count(&self, contact: &str) -> anyhow::Result<u64> {
        let msgs = self.read_messages_raw(contact).await?;
        Ok(msgs
            .iter()
            .filter(|m| !m.is_sent && m.status == "received")
            .count() as u64)
    }

    /// Get all contacts that have stored messages, with unread count.
    pub async fn get_all_contacts_with_unread(&self) -> anyhow::Result<Vec<(String, u64)>> {
        let _guard = self.lock.lock().await;
        let users_dir = self.base_path.join("users");
        let mut result = Vec::new();
        let mut entries = match tokio::fs::read_dir(&users_dir).await {
            Ok(e) => e,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(result),
            Err(e) => return Err(e.into()),
        };
        while let Some(entry) = entries.next_entry().await? {
            if entry.file_type().await?.is_dir() {
                if let Some(name) = entry.file_name().to_str() {
                    let msgs = self.read_messages_raw(name).await.unwrap_or_default();
                    let unread = msgs
                        .iter()
                        .filter(|m| !m.is_sent && m.status == "received")
                        .count() as u64;
                    result.push((name.to_string(), unread));
                }
            }
        }
        result.sort_by(|a, b| a.0.cmp(&b.0));
        Ok(result)
    }

    /// Get conversation summaries across all contacts, sorted by most recent first.
    pub async fn get_conversations(&self) -> anyhow::Result<Vec<ConversationSummary>> {
        let _guard = self.lock.lock().await;
        let users_dir = self.base_path.join("users");
        let mut result = Vec::new();
        let mut entries = match tokio::fs::read_dir(&users_dir).await {
            Ok(e) => e,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(result),
            Err(e) => return Err(e.into()),
        };
        while let Some(entry) = entries.next_entry().await? {
            if entry.file_type().await?.is_dir() {
                if let Some(name) = entry.file_name().to_str() {
                    let msgs = self.read_messages_raw(name).await.unwrap_or_default();
                    if let Some(last) = msgs.last() {
                        let unread = msgs
                            .iter()
                            .filter(|m| !m.is_sent && m.status == "received")
                            .count() as u64;
                        result.push(ConversationSummary {
                            contact_address: name.to_string(),
                            last_content: last.content.clone(),
                            last_timestamp: last.timestamp,
                            last_is_sent: last.is_sent,
                            unread_count: unread,
                        });
                    }
                }
            }
        }
        result.sort_by(|a, b| b.last_timestamp.cmp(&a.last_timestamp));
        Ok(result)
    }

    // ---------------------------------------------------------------
    //  Profile (personal info — local only)
    // ---------------------------------------------------------------

    /// Save or overwrite the local profile for an address.
    pub async fn save_profile(&self, address: &str, profile: &UserProfile) -> anyhow::Result<()> {
        self.ensure_user_dir(address).await?;
        self.store_json(address, "profile.json", profile).await?;
        let mut cache = self.profile_cache.lock().await;
        cache.insert(address.to_string(), profile.clone());
        Ok(())
    }

    /// Load the local profile for an address. Returns the default profile if none exists.
    /// Uses an in-memory cache to avoid repeated disk reads.
    pub async fn load_profile(&self, address: &str) -> anyhow::Result<UserProfile> {
        {
            let cache = self.profile_cache.lock().await;
            if let Some(profile) = cache.get(address) {
                return Ok(profile.clone());
            }
        }
        let profile: Option<UserProfile> = self.load_json(address, "profile.json").await?;
        let profile = profile.unwrap_or_else(|| {
            let mut p = UserProfile::default();
            p.added_at = chrono::Utc::now().timestamp();
            p
        });
        let mut cache = self.profile_cache.lock().await;
        cache.insert(address.to_string(), profile.clone());
        Ok(profile)
    }

    // ---------------------------------------------------------------
    //  Images
    // ---------------------------------------------------------------

    /// Save an image file to `<user_dir>/images/<name>`.
    /// The name should include the extension (e.g. "avatar.jpg", "photo_001.png").
    pub async fn save_image(&self, address: &str, name: &str, data: &[u8]) -> anyhow::Result<()> {
        self.ensure_images_dir(address).await?;
        self.store_file(address, &format!("images/{}", name), data)
            .await
    }

    /// Load an image file from `<user_dir>/images/<name>`.
    pub async fn load_image(&self, address: &str, name: &str) -> anyhow::Result<Option<Vec<u8>>> {
        self.read_file(address, &format!("images/{}", name)).await
    }

    /// Delete an image file from `<user_dir>/images/<name>`.
    pub async fn delete_image(&self, address: &str, name: &str) -> anyhow::Result<bool> {
        self.delete_file(address, &format!("images/{}", name)).await
    }

    /// List all stored image file names for an address.
    pub async fn list_images(&self, address: &str) -> anyhow::Result<Vec<String>> {
        self.list_files(address, "images").await
    }
}
