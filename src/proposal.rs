/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::collections::HashSet;
use std::convert::TryInto;

use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::json_types::{ValidAccountId, U128, U64};
use near_sdk::serde::{Deserialize, Serialize};
use near_sdk::{env, AccountId, Balance, Promise};

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub struct Voter {
    pub account: AccountId,
    /// voting power, must be in [1...10000]
    pub power: u16,
}

/// Internal Action representation
#[derive(BorshSerialize, BorshDeserialize)]
pub enum ActionInt {
    Transfer { dest: AccountId, amount: Balance },
    Delete { dest: AccountId },
}

/// Action is a JSON compatible type for encodidng actions
#[cfg(not(test))]
#[derive(Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub enum Action {
    Transfer { dest: ValidAccountId, amount: U128 },
    Delete { dest: ValidAccountId },
}

#[cfg(test)]
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(crate = "near_sdk::serde")]
pub enum Action {
    Transfer { dest: ValidAccountId, amount: U128 },
    Delete { dest: ValidAccountId },
}

impl Action {
    /// Creates `ActionInt` from this object.
    fn to_aint(&self) -> ActionInt {
        match self {
            Action::Transfer { dest, amount } => ActionInt::Transfer {
                dest: dest.clone().into(),
                amount: amount.clone().into(),
            },
            Action::Delete { dest } => ActionInt::Delete {
                dest: dest.clone().into(),
            },
        }
    }
}

impl Into<Action> for ActionInt {
    fn into(self) -> Action {
        match self {
            ActionInt::Transfer { dest, amount } => Action::Transfer {
                dest: dest.try_into().unwrap(),
                amount: amount.into(),
            },
            ActionInt::Delete { dest } => Action::Delete {
                dest: dest.try_into().unwrap(),
            },
        }
    }
}

#[derive(BorshSerialize, BorshDeserialize)]
pub struct Proposal {
    pub proposer: AccountId,
    pub description: String,
    pub action: ActionInt,
    pub voters: HashSet<AccountId>,
    /// block number when voting started
    pub voting_start: u64,
    pub voting_end: u64,
    pub votes_for: u32,
    pub votes_against: u32,
    pub execute_before: u64,
    pub executed: bool,
}

impl Proposal {
    pub fn vote(&mut self, voter: &Voter, vote_yes: bool) {
        let b = env::block_index();
        assert!(
            self.voting_start <= b && self.voting_end >= b,
            "voting is not active"
        );
        assert!(
            self.voters.insert(voter.account.clone()),
            "you already voted"
        );
        let p: u32 = voter.power.into();
        if vote_yes {
            self.votes_for += p;
        } else {
            self.votes_against += p;
        }
    }

    pub fn execute(&mut self, min_support: u32) -> Promise {
        let b = env::block_index();
        assert!(
            self.voting_end < b && b <= self.execute_before,
            "proposal can be executed only between {} and {} block",
            self.voting_end + 1,
            self.execute_before
        );
        assert!(
            self.votes_for >= min_support,
            "proposal didn't get enough support (got {}, required: {})",
            self.votes_for,
            min_support
        );
        assert!(
            self.votes_for > self.votes_against,
            "proposal didn't pass (votes_for: {}, votes_against: {})",
            self.votes_for,
            self.votes_against
        );
        assert!(!self.executed, "proposal already executed");
        self.executed = true;
        match &self.action {
            ActionInt::Transfer { dest, amount } => Promise::new(dest.clone()).transfer(*amount),
            ActionInt::Delete { dest } => {
                Promise::new(env::current_account_id()).delete_account(dest.clone())
            }
        }
    }
}

#[cfg(test)]
#[derive(Serialize, Deserialize, Clone)]
#[serde(crate = "near_sdk::serde")]
pub struct NewProposal {
    pub action: Action,
    pub description: String,
    /// block number when voting started
    pub voting_start: U64,
    /// voting duration in number of blocks
    pub voting_duration: u32,
    /// last block number when the proposal can be executed. Must be bigger than
    /// `voting_start + voting_duration`
    pub execute_before: U64,
}

#[cfg(not(test))]
#[derive(Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub struct NewProposal {
    pub action: Action,
    pub description: String,
    /// block number when voting started
    pub voting_start: U64,
    /// voting duration in number of blocks
    pub voting_duration: u32,
    /// last block number when the proposal can be executed. Must be bigger than
    /// `voting_start + voting_duration`
    pub execute_before: U64,
}

impl NewProposal {
    pub fn into_proposal(&self, min_duration: u32, max_duration: u32) -> Proposal {
        let voting_start = u64::from(self.voting_start);
        let execute_before = u64::from(self.execute_before);
        assert!(
            voting_start > env::block_index(),
            "voting_start must be after current block"
        );
        assert!(
            min_duration <= self.voting_duration && self.voting_duration <= max_duration,
            "voting duration must be between {} and {}",
            min_duration,
            max_duration
        );
        let voting_end = voting_start + u64::from(self.voting_duration);
        assert!(
            execute_before > voting_end,
            "execute_before must be after voting end"
        );
        return Proposal {
            proposer: env::predecessor_account_id(),
            description: self.description.clone(),
            action: self.action.to_aint(),
            voters: HashSet::new(),
            voting_start,
            voting_end,
            votes_for: 0,
            votes_against: 0,
            execute_before,
            executed: false,
        };
    }
}

/// JSON compatible return type for Proposal
#[cfg(not(test))]
#[derive(Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub struct ProposalOut {
    pub action: Action,
    pub description: String,
    /// block number when voting started
    pub voting_start: U64,
    /// voting duration in number of blocks
    pub voting_end: U64,
    pub votes_for: u32,
    pub votes_against: u32,
    pub execute_before: U64,
    pub executed: bool,
}

#[cfg(test)]
#[derive(Serialize, Deserialize, Debug, PartialEq)]
#[serde(crate = "near_sdk::serde")]
pub struct ProposalOut {
    pub action: Action,
    pub description: String,
    /// block number when voting started
    pub voting_start: U64,
    /// voting duration in number of blocks
    pub voting_end: U64,
    pub votes_for: u32,
    pub votes_against: u32,
    pub execute_before: U64,
    pub executed: bool,
}

impl From<Proposal> for ProposalOut {
    fn from(p: Proposal) -> ProposalOut {
        ProposalOut {
            action: p.action.into(),
            description: p.description,
            voting_start: p.voting_start.into(),
            voting_end: p.voting_end.into(),
            votes_for: p.votes_for,
            votes_against: p.votes_against,
            execute_before: p.execute_before.into(),
            executed: p.executed,
        }
    }
}

#[inline]
pub fn assert_valid_account(a: &AccountId) {
    assert!(
        env::is_valid_account_id(a.as_bytes()),
        "account {} is not valid",
        a
    )
}
