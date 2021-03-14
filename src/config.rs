/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use near_sdk::json_types::U128;
use near_sdk::serde::{Deserialize, Serialize};
use near_sdk::AccountId;

// use crate::Voter;

/// Contract settings
#[derive(Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub struct Settings {
    pub deployer_id: AccountId,
    pub members: String, // Vec<Voter>,
    pub min_support: u32,
    pub min_duration: u32,
    pub max_duration: u32,
    pub min_bond: U128,
}
