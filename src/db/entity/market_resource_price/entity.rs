use chrono::Datelike;
use sea_orm::entity::prelude::*;

use crate::db::entity::types::{PricingUnit, ResourceType, ResourceUnit};

/// 资源市场价格（慢变量，通常按月更新）
/// 作为 epoch / tick 资源权重计算的定价基础
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "market_resource_price")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,

    /* ---------- resource ---------- */
    pub resource_type: ResourceType, // CPU / Bandwidth / IP / Storage / API ...

    /* ---------- geo ---------- */
    pub country: String, // ISO-3166-1 alpha-2
    pub city: String,    // canonical city name

    /* ---------- pricing model ---------- */
    pub pricing_unit: PricingUnit, // TimeBased / QuantityBased / Hybrid
    pub unit: ResourceUnit,        // IP / GB / Mbps / Core / Request ...

    /* ---------- market price ---------- */
    pub price_per_unit_per_month: u16, // 定点数，单位： 美分,一般都不会超过1000美元
    pub local_avg_income: u16,         // 本地月平均收入（定点）

    /* ---------- global reference ---------- */
    pub global_avg_price: u16,  // 全局参考价格（同 unit）
    pub global_avg_income: u16, // 全局平均收入

    /* ---------- validity ---------- */
    pub valid_from_epoch: i64,
    pub valid_to_epoch: i64, // 可滚动，或预留

    /* ---------- chain ---------- */
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

impl Model {
    /// 计算基础资源权重（不涉及 tick/epoch，只是基础权重）
    pub fn base_weight(&self) -> u16 {
        // 使用 u32 暂存乘法，避免 u16 溢出
        let weight = ((self.global_avg_income as u32) * (self.global_avg_price as u32))
            / (self.local_avg_income as u32);

        weight as u16
    }

    /// 计算 tick/epoch 权重（仅使用 TimeBased 或 QuantityBased）
    /// days_in_month: 当资源按时间计算时需要
    pub fn compute_weight(&self, days_in_month: u16) -> u16 {
        match self.pricing_unit {
            PricingUnit::TimeBased => {
                // 按时间分配
                let daily_price = (self.price_per_unit_per_month as u32) / (days_in_month as u32);
                (((self.base_weight() as u32) * daily_price)
                    / (self.price_per_unit_per_month as u32)) as u16
            }
            PricingUnit::QuantityBased => {
                // 按计量分配，直接使用基础权重
                self.base_weight()
            }
        }
    }

    /// 静态函数：根据 id 获取资源权重，自动计算当月天数
    pub async fn get_weight(db: &DatabaseConnection, id: i64) -> anyhow::Result<u16> {
        // 查数据库获取 Model 实例
        let resource: Option<Model> = Entity::find().filter(Column::Id.eq(id)).one(db).await?;

        let resource = resource.ok_or_else(|| anyhow::anyhow!("Resource id {} not found", id))?;

        // 根据当前月份获取天数
        let now = chrono::Utc::now();
        let year = now.year();

        let days_in_month = match now.month() {
            2 => {
                // 闰年判断
                if (year % 4 == 0 && year % 100 != 0) || year % 400 == 0 {
                    29
                } else {
                    28
                }
            }
            4 | 6 | 9 | 11 => 30,
            _ => 31,
        };

        // 调用实例方法 compute_weight
        Ok(resource.compute_weight(days_in_month))
    }
}
