#[cfg(test)]
mod tests {
    use std::{
        collections::HashSet,
        hash::{DefaultHasher, Hash, Hasher},
    };

    use aex::tcp::types::{Codec, Command};
    use bincode::{config, decode_from_slice, encode_to_vec};
    use zz_p2p::protocols::command::{Action, Entity, P2PCommand};

    #[test]
    fn test_entity_and_action_enums() {
        // 覆盖所有 Entity 变体
        let entities = vec![
            Entity::Node,
            Entity::Message,
            Entity::Witness,
            Entity::Telephone,
            Entity::File,
        ];
        for e in entities {
            let cloned = e.clone();
            assert_eq!(e, cloned);
            println!("{:?}", e); // 触发 Debug
        }

        // 覆盖所有 Action 变体
        let actions = vec![
            Action::OnLine,
            Action::OnLineAck,
            Action::OffLine,
            Action::Ack,
            Action::Update,
            Action::SendText,
            Action::SendBinary,
            Action::Tick,
            Action::Check,
            Action::Call,
            Action::HangUp,
            Action::Accept,
            Action::Reject,
        ];
        for a in actions {
            let cloned = a.clone();
            assert_eq!(a, cloned);
            println!("{:?}", a); // 触发 Debug
        }
    }

    #[test]
    fn test_p2p_command_new_and_getters() {
        let data = vec![1, 2, 3, 4];
        let cmd = P2PCommand::new(Entity::Node, Action::OnLine, data.clone());

        // 覆盖 Command Trait 的方法
        assert_eq!(cmd.data(), &data);

        // 验证 P2PCommand 内部结构
        assert_eq!(cmd.entity, Entity::Node);
        assert_eq!(cmd.action, Action::OnLine);

        // 触发 Debug 和 Clone
        let cmd_clone = cmd.clone();
        assert_eq!(cmd_clone, cmd);
        let _ = format!("{:?}", cmd);
    }

    #[test]
    fn test_to_u32_logic() {
        // 验证 ID 生成算法：((action as u32) << 8) | (entity as u32)
        // Entity::Node (1), Action::OnLine (1) -> (1 << 8) | 1 = 257
        let id = P2PCommand::to_u32(Entity::Node, Action::OnLine);
        assert_eq!(id, 257);

        // 使用 Command Trait 的 id() 方法验证
        let cmd = P2PCommand::new(Entity::Node, Action::OnLine, vec![]);
        assert_eq!(cmd.id(), 257);

        // 验证不同的 ID
        // Entity::File (5), Action::Reject (13) -> (13 << 8) | 5 = 3333
        let id_complex = P2PCommand::to_u32(Entity::File, Action::Reject);
        assert_eq!(id_complex, (13 << 8) | 5);
    }

    #[test]
    fn test_serialization_and_codec() {
        let cmd = P2PCommand::new(Entity::Message, Action::SendText, b"hello".to_vec());

        // 验证 Serde (Serialize/Deserialize)
        let serialized = serde_json::to_string(&cmd).unwrap();
        let deserialized: P2PCommand = serde_json::from_str(&serialized).unwrap();
        assert_eq!(cmd, deserialized);

        // 验证 Bincode (Encode/Decode)
        let config = bincode::config::standard();
        let encoded = bincode::encode_to_vec(&cmd, config).expect("Encode failed");
        let (decoded, _): (P2PCommand, usize) =
            bincode::decode_from_slice(&encoded, config).expect("Decode failed");
        assert_eq!(cmd, decoded);
    }

    #[test]
    fn test_enum_hash_and_eq() {
        // 覆盖 Hash 特性，确保能在集合中使用
        let mut set = HashSet::new();
        set.insert(Entity::Node);
        set.insert(Entity::Node);
        assert_eq!(set.len(), 1);

        let mut action_set = HashSet::new();
        action_set.insert(Action::OnLine);
        assert!(action_set.contains(&Action::OnLine));
    }

    #[test]
    fn test_p2p_command_codec_coverage() {
        // 1. 准备测试数据，确保涵盖 enum 和 Vec<u8>
        let original_data = vec![0xDE, 0xAD, 0xBE, 0xEF];
        let cmd = P2PCommand::new(Entity::Telephone, Action::Accept, original_data.clone());

        // 2. 显式测试 bincode 的 Encode 和 Decode 逻辑
        let config = config::standard();

        // 触发 Encode 宏生成的代码
        let encoded = encode_to_vec(&cmd, config).expect("Failed to encode P2PCommand");

        // 确保编码后的数据不为空
        assert!(!encoded.is_empty());

        // 触发 Decode 宏生成的代码
        let (decoded, len): (P2PCommand, usize) =
            decode_from_slice(&encoded, config).expect("Failed to decode P2PCommand");

        // 3. 验证字段一致性以确保 decode 逻辑全覆盖
        assert_eq!(len, encoded.len());
        assert_eq!(decoded.entity, Entity::Telephone);
        assert_eq!(decoded.action, Action::Accept);
        assert_eq!(decoded.data, original_data);
        assert_eq!(decoded, cmd);

        // 4. 显式调用 Codec trait 方法
        // 尽管目前 Codec 是空的，但调用它能确保 trait 定义被覆盖
        fn dummy_codec_user<T: Codec>(_: T) {}
        dummy_codec_user(cmd);
    }

    #[test]
    fn test_bincode_error_path() {
        // 覆盖 Decode 可能出现的错误路径（非法字节流）
        let config = config::standard();
        let corrupt_data = vec![0u8; 2]; // 长度不足以解析成 P2PCommand
        let result: Result<(P2PCommand, usize), _> = decode_from_slice(&corrupt_data, config);

        assert!(result.is_err(), "Decoding corrupt data should fail");
    }

    #[test]
    fn test_all_variants_encode_decode() {
        // 为了确保所有 Enum 变体的 Encode/Decode 分支都被覆盖
        // 我们需要至少测试每一个 Enum 的第一个和最后一个值
        let cases = vec![
            (Entity::Node, Action::OnLine),
            (Entity::File, Action::Reject),
        ];

        for (e, a) in cases {
            let cmd = P2PCommand::new(e, a, vec![1]);
            let encoded = bincode::encode_to_vec(&cmd, bincode::config::standard()).unwrap();
            let (decoded, _): (P2PCommand, usize) =
                bincode::decode_from_slice(&encoded, bincode::config::standard()).unwrap();
            assert_eq!(decoded.entity, e);
            assert_eq!(decoded.action, a);
        }
    }

    #[test]
    fn test_p2p_command_encode_full_coverage() {
        // 1. 构造测试数据（覆盖第一个枚举值和有数据的 Vec）
        let cmd_start = P2PCommand::new(Entity::Node, Action::OnLine, vec![1, 2, 3]);

        // 2. 执行编码（触发 Encode 派生宏的代码路径）
        let config = config::standard();
        let encoded_start = encode_to_vec(&cmd_start, config).expect("Encode failed for cmd_start");

        // 验证编码结果不为空且包含数据长度信息
        assert!(encoded_start.len() > 3);

        // 3. 构造测试数据（覆盖最后一个枚举值和空的 Vec）
        let cmd_end = P2PCommand::new(Entity::File, Action::Reject, vec![]);

        let encoded_end = encode_to_vec(&cmd_end, config).expect("Encode failed for cmd_end");

        // 验证不同枚举值产生的编码结果不同
        assert_ne!(encoded_start, encoded_end);

        // 4. 验证 ID 逻辑（覆盖 to_u32 内部位移逻辑）
        assert_eq!(cmd_start.id(), 257); // (1 << 8) | 1
        assert_eq!(cmd_end.id(), (13 << 8) | 5);
    }

    #[test]
    fn test_trait_impl_coverage() {
        let cmd = P2PCommand::new(Entity::Message, Action::SendText, vec![0xFF]);

        // 显式覆盖 Command Trait 的方法实现
        // let trait_ptr: &dyn Command + 'static= &cmd;
        assert_eq!(cmd.id(), (6 << 8) | 2);
        assert_eq!(cmd.data(), &vec![0xFF]);

        // 显式触发 Codec 标记（虽然是空的，但 impl 块需要被触达）
        fn check_codec<T: Codec>(_: T) {}
        check_codec(cmd);
    }

    #[test]
    fn test_all_entity_variants_codec() {
        let config = config::standard();
        let all_entities = vec![
            Entity::Node,
            Entity::Message,
            Entity::Witness,
            Entity::Telephone,
            Entity::File,
        ];

        for entity in all_entities {
            // 触发 Encode
            let encoded = encode_to_vec(entity, config).expect("Entity encode failed");
            // 触发 Decode
            let (decoded, _): (Entity, usize) =
                decode_from_slice(&encoded, config).expect("Entity decode failed");

            assert_eq!(entity, decoded);
        }
    }

    #[test]
    fn test_all_action_variants_codec() {
        let config = config::standard();
        let all_actions = vec![
            Action::OnLine,
            Action::OnLineAck,
            Action::OffLine,
            Action::Ack,
            Action::Update,
            Action::SendText,
            Action::SendBinary,
            Action::Tick,
            Action::Check,
            Action::Call,
            Action::HangUp,
            Action::Accept,
            Action::Reject,
        ];

        for action in all_actions {
            // 触发 Encode
            let encoded = encode_to_vec(action, config).expect("Action encode failed");
            // 触发 Decode
            let (decoded, _): (Action, usize) =
                decode_from_slice(&encoded, config).expect("Action decode failed");

            assert_eq!(action, decoded);
        }
    }

    #[test]
    fn test_p2p_command_full_struct_codec() {
        let config = config::standard();
        // 测试一个带有数据的完整结构体，确保 P2PCommand 的 Encode/Decode 宏也被覆盖
        let cmd = P2PCommand::new(Entity::Message, Action::SendBinary, vec![0x0, 0x1, 0x2]);

        let encoded = encode_to_vec(&cmd, config).unwrap();
        let (decoded, _): (P2PCommand, usize) = decode_from_slice(&encoded, config).unwrap();

        assert_eq!(cmd, decoded);
    }

    #[test]
    fn test_absolute_coverage() {
        let cfg = config::standard();

        // 1. 覆盖 Entity 所有变体的 Encode/Decode/Debug/Hash/PartialEq
        let entities = [
            Entity::Node,
            Entity::Message,
            Entity::Witness,
            Entity::Telephone,
            Entity::File,
        ];

        for e in entities {
            // Encode & Decode (Bincode 路径)
            let enc = encode_to_vec(e, cfg).unwrap();
            let (dec, _): (Entity, usize) = decode_from_slice(&enc, cfg).unwrap();
            assert_eq!(e, dec);

            // Debug (覆盖 derive Debug)
            let _ = format!("{:?}", e);

            // Hash (覆盖 derive Hash)
            let mut h = DefaultHasher::new();
            e.hash(&mut h);
            let _ = h.finish();
        }

        // 2. 覆盖 Action 所有变体的 Encode/Decode/Debug/Hash/PartialEq
        let actions = [
            Action::OnLine,
            Action::OnLineAck,
            Action::OffLine,
            Action::Ack,
            Action::Update,
            Action::SendText,
            Action::SendBinary,
            Action::Tick,
            Action::Check,
            Action::Call,
            Action::HangUp,
            Action::Accept,
            Action::Reject,
        ];

        for a in actions {
            let enc = encode_to_vec(a, cfg).unwrap();
            let (dec, _): (Action, usize) = decode_from_slice(&enc, cfg).unwrap();
            assert_eq!(a, dec);
            let _ = format!("{:?}", a);
            let mut h = DefaultHasher::new();
            a.hash(&mut h);
            let _ = h.finish();
        }

        // 3. 覆盖 P2PCommand 的所有 Trait 和方法
        let data = vec![1, 2, 3];
        let cmd = P2PCommand::new(Entity::Node, Action::OnLine, data.clone());

        // Bincode Encode/Decode
        let enc_cmd = encode_to_vec(&cmd, cfg).unwrap();
        let (dec_cmd, _): (P2PCommand, usize) = decode_from_slice(&enc_cmd, cfg).unwrap();
        assert_eq!(cmd, dec_cmd);

        // Command Trait 方法
        use aex::tcp::types::Command;
        assert_eq!(cmd.id(), P2PCommand::to_u32(Entity::Node, Action::OnLine));
        assert_eq!(cmd.data(), &data);

        // Codec Trait 显式覆盖
        fn trigger_codec<T: Codec>(_: T) {}
        trigger_codec(cmd.clone());

        // Debug & Clone
        let _ = format!("{:?}", cmd);
        let _ = cmd.clone();
    }

    #[test]
    fn test_to_u32_edge_cases() {
        // 覆盖 ID 计算逻辑
        assert_eq!(P2PCommand::to_u32(Entity::Node, Action::OnLine), 257);
        assert_eq!(P2PCommand::to_u32(Entity::File, Action::Reject), 3333);
    }
}
