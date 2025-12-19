use std::{
    fs,
    path::PathBuf,
};

use zz_account::address::FreeWebMovementAddress;

use crate::{consts::{
    DEFAULT_APP_DIR,
    DEFAULT_APP_DIR_ADDRESS_JSON_FILE,
    DEFAULT_APP_DIR_SERVER_LIST_JSON_FILE,
}, nodes::record::NodeRecord};

#[derive(Debug, Clone)]
pub struct Storeage {
    app_dir: PathBuf,
    address_file: PathBuf,
    server_list_file: PathBuf,
}

impl Storeage {
    pub fn new(
        data_dir: Option<&str>,
        address_file_name: Option<&str>,
        server_list_file_name: Option<&str>,
    ) -> Self {
        let app_dir = if let Some(dir) = data_dir {
            PathBuf::from(dir)
        } else {
            dirs_next::data_dir()
                .map(|dir| dir.join(DEFAULT_APP_DIR))
                .unwrap_or_else(|| PathBuf::from(DEFAULT_APP_DIR))
        };

        let _ = fs::create_dir_all(&app_dir);

        let address_file =
            app_dir.join(address_file_name.unwrap_or(DEFAULT_APP_DIR_ADDRESS_JSON_FILE));

        let server_list_file =
            app_dir.join(server_list_file_name.unwrap_or(DEFAULT_APP_DIR_SERVER_LIST_JSON_FILE));

        Storeage {
            app_dir,
            address_file,
            server_list_file,
        }
    }

    /* ------------------ address ------------------ */

    pub fn save_address(&self, address: &FreeWebMovementAddress) -> anyhow::Result<()> {
        let json = serde_json::to_vec_pretty(address)?;
        fs::write(&self.address_file, json)?;
        Ok(())
    }

    pub fn read_address(&self) -> anyhow::Result<Option<FreeWebMovementAddress>> {
        if !self.address_file.exists() {
            return Ok(None);
        }

        let bytes = fs::read(&self.address_file)?;
        let address = serde_json::from_slice(&bytes)?;
        Ok(Some(address))
    }

    /* ------------------ server list ------------------ */

    pub fn save_server_list(
        &self,
        servers: Vec<NodeRecord>,
    ) -> anyhow::Result<()> {
        let json = serde_json::to_string_pretty(&servers).unwrap();
        fs::write(&self.server_list_file, json).unwrap();
        Ok(())
    }

    pub fn read_server_list(
        &self,
    ) -> anyhow::Result<Vec<NodeRecord>> {
        if !self.server_list_file.exists() {
            return Ok(Default::default());
        }

        let content = fs::read_to_string(&self.server_list_file).unwrap();
        Ok(serde_json::from_str(&content).unwrap())
    }

    /* ------------------ getters ------------------ */

    pub fn address_path(&self) -> &PathBuf {
        &self.address_file
    }

    pub fn server_list_path(&self) -> &PathBuf {
        &self.server_list_file
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use zz_account::address::FreeWebMovementAddress;
    use crate::protocols::defines::ProtocolCapability;
    use chrono::Utc;
    use std::net::SocketAddr;

    fn create_storage(tmp: &TempDir) -> Storeage {
        Storeage::new(Some(tmp.path().to_str().unwrap()), None, None)
    }

    #[test]
    fn test_read_address_when_not_exists() -> anyhow::Result<()> {
        let tmp = TempDir::new()?;
        let storage = create_storage(&tmp);

        let addr = storage.read_address()?;
        assert!(addr.is_none());
        Ok(())
    }

    #[test]
    fn test_save_and_read_address() -> anyhow::Result<()> {
        let tmp = TempDir::new()?;
        let storage = create_storage(&tmp);

        let address = FreeWebMovementAddress::random();
        storage.save_address(&address)?;

        let loaded = storage.read_address()?.expect("address should exist");
        assert_eq!(address.to_string(), loaded.to_string());
        Ok(())
    }

    #[test]
    fn test_read_server_list_when_not_exists() -> anyhow::Result<()> {
        let tmp = TempDir::new()?;
        let storage = create_storage(&tmp);

        let list = storage.read_server_list()?;
        assert!(list.is_empty());
        Ok(())
    }

    #[test]
    fn test_save_and_read_empty_server_list() -> anyhow::Result<()> {
        let tmp = TempDir::new()?;
        let storage = create_storage(&tmp);

        let servers: Vec<NodeRecord> = Vec::new();
        storage.save_server_list(servers.clone())?;

        let loaded = storage.read_server_list()?;
        assert!(loaded.is_empty());
        Ok(())
    }

    #[test]
    fn test_save_and_read_server_list() -> anyhow::Result<()> {
        let tmp = TempDir::new()?;
        let storage = create_storage(&tmp);

        let now = Utc::now();
        let servers = vec![
            NodeRecord {
                endpoint: "127.0.0.1:18000".parse::<SocketAddr>()?,
                protocols: ProtocolCapability::TCP,
                first_seen: now,
                last_seen: now,
                last_disappeared: None,
                reachability_score: 1.0,
            },
            NodeRecord {
                endpoint: "192.168.1.10:9000".parse::<SocketAddr>()?,
                protocols: ProtocolCapability::UDP,
                first_seen: now,
                last_seen: now,
                last_disappeared: None,
                reachability_score: 0.8,
            },
        ];

        storage.save_server_list(servers.clone())?;
        let loaded = storage.read_server_list()?;

        assert_eq!(servers.len(), loaded.len());
        for server in servers.iter() {
            assert!(loaded.iter().any(|s| s.endpoint == server.endpoint));
        }

        Ok(())
    }

    #[test]
    fn test_storage_creates_directory() -> anyhow::Result<()> {
        let tmp = TempDir::new()?;
        let dir = tmp.path().join("app_data");

        let storage = Storeage::new(Some(dir.to_str().unwrap()), None, None);

        assert!(dir.exists());
        assert!(storage.address_path().parent().unwrap().exists());
        assert!(storage.server_list_path().parent().unwrap().exists());

        Ok(())
    }
}
