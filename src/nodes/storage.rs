use std::{
    fs,
    path::PathBuf,
};

use serde::{Deserialize, Serialize};

use zz_account::address::FreeWebMovementAddress;

use crate::consts::{
    DEFAULT_APP_DIR,
    DEFAULT_APP_DIR_ADDRESS_JSON_FILE,
    DEFAULT_APP_DIR_SERVER_LIST_JSON_FILE,
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct ServerNode {
    pub addr: std::net::SocketAddr,
}

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
        servers: &std::collections::HashSet<ServerNode>,
    ) -> anyhow::Result<()> {
        let json = serde_json::to_vec_pretty(servers)?;
        fs::write(&self.server_list_file, json)?;
        Ok(())
    }

    pub fn read_server_list(
        &self,
    ) -> anyhow::Result<std::collections::HashSet<ServerNode>> {
        if !self.server_list_file.exists() {
            return Ok(Default::default());
        }

        let bytes = fs::read(&self.server_list_file)?;
        let list = serde_json::from_slice(&bytes)?;
        Ok(list)
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
    use std::collections::HashSet;
    use tempfile::TempDir;
    use zz_account::address::FreeWebMovementAddress;

    fn create_storage(tmp: &TempDir) -> Storeage {
        Storeage::new(
            Some(tmp.path().to_str().unwrap()),
            None,
            None,
        )
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

        let list: HashSet<ServerNode> = HashSet::new();
        storage.save_server_list(&list)?;

        let loaded = storage.read_server_list()?;
        assert!(loaded.is_empty());

        Ok(())
    }

    #[test]
    fn test_save_and_read_server_list() -> anyhow::Result<()> {
        let tmp = TempDir::new()?;
        let storage = create_storage(&tmp);

        let mut list = HashSet::new();
        list.insert(ServerNode {
            addr: "127.0.0.1:18000".parse().unwrap(),
        });
        list.insert(ServerNode {
            addr: "192.168.1.10:9000".parse().unwrap(),
        });

        storage.save_server_list(&list)?;

        let loaded = storage.read_server_list()?;
        assert_eq!(list.len(), loaded.len());
        assert_eq!(list, loaded);

        Ok(())
    }

    #[test]
    fn test_server_list_uniqueness() -> anyhow::Result<()> {
        let tmp = TempDir::new()?;
        let storage = create_storage(&tmp);

        let mut list = HashSet::new();
        list.insert(ServerNode {
            addr: "8.8.8.8:53".parse().unwrap(),
        });
        list.insert(ServerNode {
            addr: "8.8.8.8:53".parse().unwrap(),
        });

        assert_eq!(list.len(), 1);

        storage.save_server_list(&list)?;
        let loaded = storage.read_server_list()?;
        assert_eq!(loaded.len(), 1);

        Ok(())
    }

    #[test]
    fn test_storage_creates_directory() -> anyhow::Result<()> {
        let tmp = TempDir::new()?;
        let dir = tmp.path().join("app_data");

        let storage = Storeage::new(
            Some(dir.to_str().unwrap()),
            None,
            None,
        );

        assert!(dir.exists());
        assert!(storage.address_path().parent().unwrap().exists());
        assert!(storage.server_list_path().parent().unwrap().exists());

        Ok(())
    }
}
