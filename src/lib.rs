/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::collections::Vector;
use near_sdk::json_types::U128;
use near_sdk::{env, log, near_bindgen, AccountId, Balance, PanicOnDefault, Promise, StorageUsage};

pub mod proposal;
use crate::proposal::*;

// a way to optimize memory management
near_sdk::setup_alloc!();

const STORAGE_PRICE_PER_BYTE: Balance = env::STORAGE_PRICE_PER_BYTE;

#[near_bindgen]
#[derive(BorshSerialize, BorshDeserialize, PanicOnDefault)]
pub struct Contract {
    /// Account of the owner.
    deployer_id: AccountId,
    signers: Vec<Voter>,
    /// minimum support (in power) to pass the call
    min_support: u32,
    /// Each proposal voting duration must be between `min_duration` and `max_duration` expressed
    /// in number of blocks. Both values must be >= 2.
    min_duration: u32,
    max_duration: u32,
    min_bond: Balance,

    next_idx: u32,
    proposals: Vector<Proposal>,
}

#[near_bindgen]
impl Contract {
    #[init]
    pub fn new(
        signers: Vec<Voter>,
        min_support: u32,
        min_duration: u32,
        max_duration: u32,
        min_bond: U128,
    ) -> Self {
        assert!(!env::state_exists(), "ERR_CONTRACT_IS_INITIALIZED");
        assert!(min_support > 0, "min_support must be positive");
        for s in &signers {
            assert_valid_account(&s.account);
        }
        assert!(
            min_duration >= 2 && max_duration > min_duration,
            "min_duration and max_duration must be at least 2"
        );
        let min_bond: u128 = min_bond.into();
        assert!(
            min_bond > STORAGE_PRICE_PER_BYTE,
            "min_bond must be bigger than {}",
            STORAGE_PRICE_PER_BYTE
        );
        Self {
            deployer_id: env::predecessor_account_id(),
            signers,
            min_support,
            min_duration,
            max_duration,
            min_bond: min_bond,
            next_idx: 0,
            proposals: Vector::new("p".into()),
        }
    }

    pub fn add_proposal(&mut self, p: NewProposal) -> u32 {
        let storage_start = env::storage_usage();
        self.proposals
            .push(&p.into_proposal(self.min_duration.into(), self.max_duration.into()));
        log!("New proposal added, id={}.", self.next_idx);
        self.next_idx += 1;
        self.refund_storage(storage_start, true);
        return self.next_idx - 1;
    }

    pub fn vote(&mut self, proposal_id: u32, vote_yes: bool) {
        let a = env::predecessor_account_id();
        let mut voter_o: Option<&Voter> = None;
        for s in &self.signers {
            if s.account == a {
                voter_o = Some(s);
                break;
            }
        }
        let voter = voter_o.expect(&format!("you ({}) are not a  valid voter", a));
        let idx: u64 = proposal_id.into();
        let p = &mut self.proposals.get(idx).expect("proposal_id not found");
        let storage_start = env::storage_usage();
        p.vote(voter, vote_yes);
        self.proposals.replace(idx, p);
        self.refund_storage(storage_start, false);
    }

    pub fn execute(&mut self, proposal_id: u32) -> Promise {
        let idx: u64 = proposal_id.into();
        let p = &mut self.proposals.get(idx).expect("proposal_id not found");
        let promise = p.execute(self.min_support);
        self.proposals.replace(idx, p);
        log!("Proposal {} executed.", proposal_id);
        return promise;
    }

    pub fn check_proposal(&self, proposal_id: u32) -> ProposalOut {
        assert!(proposal_id < self.next_idx, "proposal_id not found");
        let idx: u64 = proposal_id.into();
        let p = self.proposals.get(idx).expect("proposal_id not found");
        p.into()
    }

    fn refund_storage(&self, initial_storage: StorageUsage, check_bond: bool) {
        let current_storage = env::storage_usage();
        let attached_deposit = env::attached_deposit();
        let refund_amount = if current_storage > initial_storage {
            let mut required_deposit =
                Balance::from(current_storage - initial_storage) * STORAGE_PRICE_PER_BYTE;
            if check_bond && required_deposit < self.min_bond {
                required_deposit = self.min_bond
            }
            assert!(
                required_deposit <= attached_deposit,
                "The required attached deposit is {}, but the given attached deposit is is {}",
                required_deposit,
                attached_deposit,
            );
            attached_deposit - required_deposit
        } else {
            attached_deposit
                + Balance::from(initial_storage - current_storage) * STORAGE_PRICE_PER_BYTE
        };
        if refund_amount > 0 {
            Promise::new(env::predecessor_account_id()).transfer(refund_amount);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use near_sdk::test_utils::{accounts, VMContextBuilder};
    use near_sdk::{testing_env, MockedBlockchain};

    const BASE_UNIT: Balance = STORAGE_PRICE_PER_BYTE * 20;

    fn setup_contract(min_support: u32) -> (VMContextBuilder, Contract) {
        let voters: Vec<Voter> = vec![
            Voter {
                account: accounts(0).into(),
                power: 2,
            },
            Voter {
                account: accounts(1).into(),
                power: 3,
            },
            Voter {
                account: accounts(2).into(),
                power: 4,
            },
        ];
        let mut context = VMContextBuilder::new();
        testing_env!(context.build());
        let contract = Contract::new(voters, min_support, 10, 20, BASE_UNIT.into());
        testing_env!(context
            .predecessor_account_id(accounts(0))
            .attached_deposit(BASE_UNIT * 2)
            .build());
        (context, contract)
    }

    fn attach_near(ctx: &mut VMContextBuilder, amount: Balance) {
        testing_env!(ctx
            .predecessor_account_id(accounts(0))
            .attached_deposit(amount)
            .build());
    }

    fn init_blockchain() {
        let context = VMContextBuilder::new();
        testing_env!(context.build());
    }

    #[test]
    #[should_panic(expected = "min_support must be positive")]
    fn test_constructor_min_support() {
        init_blockchain();
        Contract::new(Vec::new(), 0, 2, 20, 10.into());
    }

    #[test]
    #[should_panic(expected = "min_duration and max_duration must be at least 2")]
    fn test_constructor_min_duration() {
        init_blockchain();
        Contract::new(Vec::new(), 10, 1, 20, 10.into());
    }

    #[test]
    #[should_panic(expected = "min_duration and max_duration must be at least 2")]
    fn test_constructor_max_duration() {
        init_blockchain();
        Contract::new(Vec::new(), 10, 2, 2, 10.into());
    }

    #[test]
    #[should_panic(expected = "min_duration and max_duration must be at least 2")]
    fn test_constructor_max_duration2() {
        init_blockchain();
        Contract::new(Vec::new(), 10, 3, 2, 10.into());
    }

    #[test]
    #[should_panic(expected = "min_bond must be bigger than 10000000000000000000")]
    fn test_constructor_min_bond() {
        init_blockchain();
        Contract::new(Vec::new(), 10, 2, 20, 10.into());
    }

    #[test]
    fn test_constructor_should_work() {
        init_blockchain();
        Contract::new(Vec::new(), 10, 2, 20, BASE_UNIT.into());
        Contract::new(Vec::new(), 10000, 2, 2000, BASE_UNIT.into());
        Contract::new(Vec::new(), 10, 20, 21, BASE_UNIT.into());
    }

    #[test]
    fn test_add_proposal() {
        let (mut ctx, mut contract) = setup_contract(5);
        attach_near(&mut ctx, BASE_UNIT * 300);
        contract.add_proposal(sample_proposal());
    }

    fn sample_proposal() -> NewProposal {
        NewProposal {
            action: Action::Transfer {
                dest: accounts(4),
                amount: 1000.into(),
            },
            description: "transfer to eugene".into(),
            voting_start: 10,
            voting_duration: 20,
            execute_before: 100,
        }
    }
}
