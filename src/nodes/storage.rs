use std::fs;
use std::path::PathBuf;

use crate::consts::{
    DEFAULT_APP_DIR,
    DEFAULT_APP_DIR_ADDRESS_JSON_FILE,
    DEFAULT_APP_DIR_EXTERNAL_SERVER_LIST_JSON_FILE,
    DEFAULT_APP_DIR_INNER_SERVER_LIST_JSON_FILE,
};

use crate::nodes::record::NodeRecord;
use zz_account::address::FreeWebMovementAddress;

#[derive(Debug, Clone)]
pub struct Storeage {
    pub app_dir: PathBuf,
    pub address_file: PathBuf,
    pub external_server_list_file: PathBuf,
    pub inner_server_list_file: PathBuf,
}

impl Storeage {
    pub fn new(
        data_dir: Option<&str>,
        address_file_name: Option<&str>,
        external_list_file_name: Option<&str>,
        inner_server_list_file_name: Option<&str>
    ) -> Self {
        let app_dir = if let Some(dir) = data_dir {
            println!("Using custom storage dir: {}", dir);
            PathBuf::from(dir)
        } else {
            println!("Using default storage dir");
            dirs_next
                ::data_dir()
                .map(|dir| dir.join(DEFAULT_APP_DIR))
                .unwrap_or_else(|| PathBuf::from(DEFAULT_APP_DIR))
        };
        println!("Storage app dir: {:?}", app_dir);

        let _ = fs::create_dir_all(&app_dir);

        let address_file = app_dir.join(
            address_file_name.unwrap_or(DEFAULT_APP_DIR_ADDRESS_JSON_FILE)
        );

        let external_server_list_file = app_dir.join(
            external_list_file_name.unwrap_or(DEFAULT_APP_DIR_EXTERNAL_SERVER_LIST_JSON_FILE)
        );

        let inner_server_list_file = app_dir.join(
            inner_server_list_file_name.unwrap_or(DEFAULT_APP_DIR_INNER_SERVER_LIST_JSON_FILE)
        );

        Storeage {
            app_dir,
            address_file,
            external_server_list_file,
            inner_server_list_file,
        }
    }

    /* ------------------ address ------------------ */

    pub fn save_address(&self, address: &FreeWebMovementAddress) -> anyhow::Result<()> {
        println!("Saving address to {:?}", &self.address_file);
        let json = serde_json::to_vec_pretty(address)?;
        fs::write(&self.address_file, json)?;
        Ok(())
    }

    pub fn read_address(&self) -> anyhow::Result<Option<FreeWebMovementAddress>> {
        println!("Reading address from {:?}", &self.address_file);
        if !self.address_file.exists() {
          println!("Address file does not exist.");
            return Ok(None);
        }

        let bytes = fs::read(&self.address_file)?;
        Ok(Some(serde_json::from_slice(&bytes)?))
    }

    /* ------------------ server list (internal) ------------------ */

    fn save_server_list_to(&self, servers: &[NodeRecord], path: &PathBuf) -> anyhow::Result<()> {
        println!("Saving server list to {:?}", path);
        let json = serde_json::to_string_pretty(servers)?;
        fs::write(path, json)?;
        Ok(())
    }

    fn read_server_list_from(&self, path: &PathBuf) -> anyhow::Result<Vec<NodeRecord>> {
        println!("Reading server list from {:?}", path);
        if !path.exists() {
            println!("Server list file does not exist.");
            return Ok(Vec::new());
        }

        let content = fs::read_to_string(path)?;
        Ok(serde_json::from_str(&content)?)
    }

    /* ------------------ external ------------------ */

    pub fn read_external_server_list(&self) -> anyhow::Result<Vec<NodeRecord>> {
        self.read_server_list_from(&self.external_server_list_file)
    }

    pub fn save_external_server_list(&self, servers: &Vec<NodeRecord>) -> anyhow::Result<()> {
        self.save_server_list_to(&servers, &self.external_server_list_file)
    }

    /* ------------------ inner ------------------ */

    pub fn read_inner_server_list(&self) -> anyhow::Result<Vec<NodeRecord>> {
        self.read_server_list_from(&self.inner_server_list_file)
    }

    pub fn save_inner_server_list(&self, servers: &Vec<NodeRecord>) -> anyhow::Result<()> {
        self.save_server_list_to(&servers, &self.inner_server_list_file)
    }

    pub fn dir(&self) -> &str {
      let str = self.app_dir.as_os_str().to_str().unwrap();
      str
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use std::net::{ IpAddr, Ipv4Addr, SocketAddr };
    use tempfile::tempdir;

    fn dummy_record(port: u16) -> NodeRecord {
        NodeRecord {
            endpoint: SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port),
            protocols: crate::protocols::defines::ProtocolCapability::TCP,
            first_seen: Utc::now(),
            last_seen: Utc::now(),
            last_disappeared: None,
            address: None,
            connected: false,
            reachability_score: 100,
        }
    }

    #[test]
    fn test_address_save_and_read() {
        let dir = tempdir().unwrap();
        let storage = Storeage::new(Some(dir.path().to_str().unwrap()), None, None, None);

        let addr = FreeWebMovementAddress::random();
        storage.save_address(&addr).unwrap();

        let loaded = storage.read_address().unwrap().unwrap();
        assert_eq!(addr.to_string(), loaded.to_string());
    }

    #[test]
    fn test_read_address_when_missing() {
        let dir = tempdir().unwrap();
        let storage = Storeage::new(Some(dir.path().to_str().unwrap()), None, None, None);

        let addr = storage.read_address().unwrap();
        assert!(addr.is_none());
    }

    #[test]
    fn test_external_server_list_roundtrip() {
        let dir = tempdir().unwrap();
        let storage = Storeage::new(Some(dir.path().to_str().unwrap()), None, None, None);

        let servers = vec![dummy_record(8080)];
        storage.save_external_server_list(&servers).unwrap();

        let loaded = storage.read_external_server_list().unwrap();
        assert_eq!(servers, loaded);
    }

    #[test]
    fn test_inner_and_external_isolation() {
        let dir = tempdir().unwrap();
        let storage = Storeage::new(Some(dir.path().to_str().unwrap()), None, None, None);

        let inner = vec![dummy_record(1000)];
        let external = vec![dummy_record(2000)];

        storage.save_inner_server_list(&inner).unwrap();
        storage.save_external_server_list(&external).unwrap();

        assert_eq!(storage.read_inner_server_list().unwrap(), inner);
        assert_eq!(storage.read_external_server_list().unwrap(), external);
    }

    #[test]
    fn test_read_empty_server_list() {
        let dir = tempdir().unwrap();
        let str = dir.path().to_str().unwrap();
        let storage = Storeage::new(Some(&str.to_string().clone()), None, None, None);

        assert!(storage.read_inner_server_list().unwrap().is_empty());
        assert!(storage.read_external_server_list().unwrap().is_empty());
    }
}
