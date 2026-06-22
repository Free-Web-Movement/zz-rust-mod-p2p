use sea_orm::ConnectionTrait;

#[macro_export]
macro_rules! create_entities {
    // 接收 db 和括号里的实体列表
    ($db:expr, ( $($entity:path),+ $(,)? )) => {{
        let mut last_err: Option<String> = None;
        $(
            match Self::create_table::<$entity>($db, $entity).await {
                Ok(_) => {}
                Err(e) => last_err = Some(e.to_string()),
            }
        )+
        if last_err.is_some() {
            Err(sea_orm::DbErr::Custom(last_err.unwrap()))
        } else {
            Ok(())
        }
    }};
}

pub trait StoreFromConnection<'a, C: ConnectionTrait + Send + Sync + 'a>: Sized {
    fn new(db: &'a C) -> Self;
}
