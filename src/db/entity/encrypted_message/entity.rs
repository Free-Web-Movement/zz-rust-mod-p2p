use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "encrypted_message")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub msg_id: String,
    pub from_address: String,
    pub to_address: String,
    pub encrypted_content: Vec<u8>,
    pub nonce: Vec<u8>,
    pub ephemeral_pk: Vec<u8>,
    pub created_at: i64,
    pub size_bytes: i64,
    pub status: String,
    pub deleted_by_sender: bool,
    pub deleted_by_receiver: bool,
    pub pre_hash: String,
    pub hash: String,
}

#[derive(Copy, Clone, Debug, EnumIter)]
pub enum Relation {}

impl RelationTrait for Relation {
    fn def(&self) -> RelationDef {
        match *self {}
    }
}

impl ActiveModelBehavior for ActiveModel {}
