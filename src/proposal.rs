use std::collections::HashSet;
use std::convert::TryInto;

use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::json_types::{ValidAccountId, U128};
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
}

/// Action is a JSON compatible type for encodidng actions
#[derive(Serialize, Deserialize, Clone)]
#[serde(crate = "near_sdk::serde")]
pub enum Action {
    Transfer { dest: ValidAccountId, amount: U128 },
}

impl Into<ActionInt> for Action {
    fn into(self) -> ActionInt {
        match self {
            Action::Transfer { dest, amount } => ActionInt::Transfer {
                dest: dest.into(),
                amount: amount.into(),
            },
        }
    }
}

impl Into<Action> for ActionInt {
    fn into(self) -> Action {
        match self {
            ActionInt::Transfer { dest, amount } => Action::Transfer {
                dest: dest.try_into().unwrap(), // ValidAccountId { 0: dest },
                amount: amount.into(),
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
            self.voting_end < b && self.execute_before < b,
            "vote can be executed only between {} and {} blocks",
            self.voting_end + 1,
            self.execute_before - 1
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
        self.executed = true;
        match &self.action {
            ActionInt::Transfer { dest, amount } => Promise::new(dest.clone()).transfer(*amount),
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub struct NewProposal {
    pub action: Action,
    pub description: String,
    /// block number when voting started
    pub voting_start: u64,
    /// voting duration in number of blocks
    pub voting_duration: u64,
    /// block number before which the proposal have to be executed. Must be bigger than
    /// `voting_start + voting_duration`
    pub execute_before: u64,
}

impl NewProposal {
    pub fn into_proposal(&self, min_duration: u64, max_duration: u64) -> Proposal {
        assert!(
            self.voting_start > env::block_index(),
            "voting_start must be after current block"
        );
        assert!(
            min_duration <= self.voting_duration && self.voting_duration <= max_duration,
            "voting duration must be between {} and {}",
            min_duration,
            max_duration
        );
        assert!(
            self.execute_before > self.voting_start + self.voting_duration,
            "execute_before must be after voting end"
        );
        return Proposal {
            proposer: env::predecessor_account_id(),
            description: self.description.clone(),
            action: self.action.clone().into(),
            voters: HashSet::new(),
            voting_start: self.voting_start,
            voting_end: self.voting_start + self.voting_duration,
            votes_for: 0,
            votes_against: 0,
            execute_before: self.execute_before,
            executed: false,
        };
    }
}

/// JSON compatible return type for Proposal
#[derive(Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub struct ProposalOut {
    pub action: Action,
    pub description: String,
    /// block number when voting started
    pub voting_start: u64,
    /// voting duration in number of blocks
    pub voting_end: u64,
    pub votes_for: u32,
    pub votes_against: u32,
    pub execute_before: u64,
    pub executed: bool,
}

impl From<Proposal> for ProposalOut {
    fn from(p: Proposal) -> ProposalOut {
        ProposalOut {
            action: p.action.into(),
            description: p.description,
            voting_start: p.voting_start,
            voting_end: p.voting_end,
            votes_for: p.votes_for,
            votes_against: p.votes_against,
            execute_before: p.execute_before,
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
