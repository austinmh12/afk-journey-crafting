//! Per-level card values, loaded from `data/cards.ron`.

use std::collections::BTreeMap;

use serde::Deserialize;

/// Values by card level, with a fallback for levels that aren't listed.
#[derive(Debug, Clone, Deserialize)]
pub struct Table<T> {
	pub default: T,
	pub levels: BTreeMap<u8, T>,
}

impl<T: Copy> Table<T> {
	pub fn get(&self, level: u8) -> T {
		self.levels.get(&level).copied().unwrap_or(self.default)
	}
}

#[derive(Debug, Clone, Copy, Deserialize)]
pub struct Range {
	pub min: i64,
	pub max: i64,
}

#[derive(Debug, Clone, Copy, Deserialize)]
pub struct ChargeValues {
	pub points: i64,
	pub charges: i64,
}

#[derive(Debug, Clone, Copy, Deserialize)]
pub struct ForgeExpertValues {
	pub value: i64,
	pub bonus: i64,
}

#[derive(Debug, Clone, Copy, Deserialize)]
pub struct OverchargeValues {
	pub amount: i64,
	pub mult: i64,
}

#[derive(Debug, Clone, Copy, Deserialize)]
pub struct FuseValues {
	pub bonus: i64,
	pub gap: i64,
}

#[derive(Debug, Clone, Copy, Deserialize)]
pub struct AddSub {
	pub add: i64,
	pub sub: i64,
}

#[derive(Debug, Clone, Copy, Deserialize)]
pub struct ValueDebuff {
	pub value: i64,
	pub debuff: i64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CardTables {
	// Kitchen
	pub cut: Table<Range>,
	pub heat_control: Table<i64>,
	pub slow_cook: Table<i64>,
	pub bake: Table<i64>,
	pub exact_cook: Table<u32>,
	pub banquet: Table<u32>,
	// Forge
	pub forge: Table<i64>,
	pub charge: Table<ChargeValues>,
	pub sharpening: Table<i64>,
	pub multi_forge: Table<i64>,
	pub heat_up: Table<i64>,
	pub reforge: Table<i64>,
	pub soulbind: Table<i64>,
	pub forge_expert: Table<ForgeExpertValues>,
	pub pyroburst: Table<i64>,
	// Alchemy
	pub overcharge: Table<OverchargeValues>,
	pub fuse: Table<FuseValues>,
	pub ingredients: Table<AddSub>,
	pub reversal: Table<i64>,
	pub enchant: Table<ValueDebuff>,
	pub overload: Table<ValueDebuff>,
	pub grind: Table<AddSub>,
}
