#[cfg(test)]
mod tests {
    use std::{collections::HashSet, net::SocketAddr, sync::Arc};
    use aex::storage::Storage;
    use tempfile::tempdir;
    use zz_account::address::FreeWebMovementAddress;
    use zz_p2p::{cli::Opt, io_storage::io_stroage_init, record::NodeRecord};

    #[tokio::test]
    async fn test_io_storage_init_persistence() {
        // 1. 创建一个持久化的临时目录（在测试运行期间存在）
        let tmp_dir = tempdir().expect("Failed to create temp dir");
        let persistent_file_path = tmp_dir.path().join("identity_address.json");
        let file_path_str = persistent_file_path.to_str().unwrap().to_string();

        // 2. 模拟底层存储引擎
        let storage = Arc::new(Storage::new(tmp_dir.path().to_str()));

        // ==========================================
        // 场景一：文件不存在，触发自动生成
        // ==========================================
        let mut opt_v1 = Opt::default();
        opt_v1.address_file = Some(file_path_str.clone()); // 传入自定义路径

        let io_storage_v1 = io_stroage_init(&opt_v1, storage.clone());
        
        // 第一次读取：因为文件不存在，f2 会生成一个随机地址
        let first_addr: FreeWebMovementAddress = io_storage_v1
            .read::<FreeWebMovementAddress>("address")
            .await
            .unwrap();

        // 验证文件是否真的写到了磁盘上
        assert!(persistent_file_path.exists(), "File should be created on disk");

        // ==========================================
        // 场景二：修改 Opt 指向同一个目录，测试读取已有文件
        // ==========================================
        let mut opt_v2 = Opt::default();
        opt_v2.address_file = Some(file_path_str.clone()); // 指向刚才生成的路径

        let io_storage_v2 = io_stroage_init(&opt_v2, storage.clone());

        // 第二次读取：此时文件已存在，逻辑应该走 f1（读取文件）
        let second_addr: FreeWebMovementAddress = io_storage_v2
            .read::<FreeWebMovementAddress>("address")
            .await
            .unwrap();

        // 【关键断言】：如果第二次读取的地址等于第一次生成的地址，说明没有触发随机生成逻辑
        assert_eq!(
            first_addr.to_string(), 
            second_addr.to_string(), 
            "Should read the existing address from disk, not generate a new one"
        );
        
        println!("Success: Persistent address confirmed as {}", second_addr);
    }

    #[tokio::test]
    async fn test_server_list_persistence_lifecycle() {
        // --- 初始化环境 ---
        let tmp_dir = tempdir().expect("Failed to create temp dir");
        let storage = Arc::new(Storage::new(tmp_dir.path().to_str()));
        
        // 定义测试用的文件路径
        let inner_file = tmp_dir.path().join("inner.json").to_str().unwrap().to_string();
        let external_file = tmp_dir.path().join("external.json").to_str().unwrap().to_string();

        // 配置 Opt 指向这些路径
        let mut opt = Opt::default();
        opt.inner_server_file = Some(inner_file.clone());
        opt.external_server_file = Some(external_file.clone());

        // ==========================================
        // 步骤 1：首次启动（文件不存在），验证自动初始化为空
        // ==========================================
        {
            let io_storage = io_stroage_init(&opt, storage.clone());

            // 读取 inner_server，预期触发 f2 生成空 HashSet
            let inner: HashSet<NodeRecord> = io_storage
                .read::<HashSet<NodeRecord>>("inner_server")
                .await
                .unwrap();
            
            assert!(inner.is_empty(), "Initial inner server list should be empty");

            // 手动构造一条记录并保存，模拟运行中添加了节点
            let mut new_set = HashSet::new();
            let endpoint = "127.0.0.1:1080".parse::<SocketAddr>().unwrap();
            let record = NodeRecord::new(endpoint);
            new_set.insert(record.clone());
            
            io_storage.save(&new_set, "inner_server").await;
            println!("Stage 1: Saved 1 record to inner_server");

            io_storage.save(&new_set, "inner_server-1").await;

        }

        // ==========================================
        // 步骤 2：第二次启动（文件已存在），验证数据被正确读取
        // ==========================================
        {
            // 重新初始化一个新的 IOStorage 实例，模拟重启
            let io_storage_v2 = io_stroage_init(&opt, storage.clone());

            // 再次读取 inner_server
            let inner_reloaded: HashSet<NodeRecord> = io_storage_v2
                .read::<HashSet<NodeRecord>>("inner_server")
                .await
                .unwrap();

            // 【关键断言】：集合不应该为空，应该包含步骤1存入的那条记录
            assert_eq!(inner_reloaded.len(), 1, "Should reload 1 record from disk");
            println!("Stage 2: Successfully reloaded {} records", inner_reloaded.len());
        }

        // ==========================================
        // 步骤 3：验证 external_server 的独立性
        // ==========================================
        {
            let io_storage = io_stroage_init(&opt, storage.clone());
            let external: HashSet<NodeRecord> = io_storage
                .read::<HashSet<NodeRecord>>("external_server")
                .await
                .unwrap();
            
            // 即使 inner 有数据，external 在没写过的情况下也应该是空的
            assert!(external.is_empty(), "External server list should still be empty");
        }
    }
}