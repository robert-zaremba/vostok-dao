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
    deployer_id: AccountId,
    members: Vec<Voter>,
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
    /**
    Creates a new multisig NEAR wallet.
    Parameters:
    + `members`: list of signers (voters) for this multisig wallet.
    + `min_support`: minimum support a proposal have to get (in power votes) to pass.
    + `min_duration`: minimum voting time (in number of blocks) for a new proposal.
    + `max_duration`: maximum voting time (in number of blocks) for a new proposal.
    + `min_bond`: minimum deposit a caller have to put to create a new proposal. It includes
       the storage fees.
    + NOTE: this parameters are binding for all proposals and can't be changed in the future. */
    #[init]
    pub fn new(
        members: Vec<Voter>,
        min_support: u32,
        min_duration: u32,
        max_duration: u32,
        min_bond: U128,
    ) -> Self {
        assert!(!env::state_exists(), "ERR_CONTRACT_IS_INITIALIZED");
        assert!(min_support > 0, "min_support must be positive");
        for s in &members {
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
            members,
            min_support,
            min_duration,
            max_duration,
            min_bond: min_bond,
            next_idx: 0,
            proposals: Vector::new("p".into()),
        }
    }

    /**
    Adds a new proposal. Can be called by anyone.
    NewProposal is validated against the Contract parameters (min_duration, max_duration)
    and the caller have to provide a deposit = max(self.min_bond, this_tx_storage_cost).
    Once validate, the proposal is appended to the list of proposals and it's `index` is
    returned. */
    pub fn add_proposal(&mut self, p: NewProposal) -> u32 {
        let storage_start = env::storage_usage();
        self.proposals
            .push(&p.into_proposal(self.min_duration, self.max_duration));
        log!("New proposal added, id={}.", self.next_idx);
        self.next_idx += 1;
        self.refund_storage(storage_start, true);
        return self.next_idx - 1;
    }

    /**
    Vote vote and signs a given proposal. proposal_id must be a valid and active proposal.
    Proposal is active if the current block is between proposal start and end block.
    Only a valid signer (member of this multisig) can vote for a proposal. Each signer
    can vote only once. */
    pub fn vote(&mut self, proposal_id: u32, vote_yes: bool) {
        let a = env::predecessor_account_id();
        let mut voter_o: Option<&Voter> = None;
        for s in &self.members {
            if s.account == a {
                voter_o = Some(s);
                break;
            }
        }
        let voter = voter_o.expect(&format!("you ({}) are not authorized to vote", a));
        let idx: u64 = proposal_id.into();
        let p = &mut self.proposals.get(idx).expect("proposal_id not found");
        let storage_start = env::storage_usage();
        p.vote(voter, vote_yes);
        self.proposals.replace(idx, p);
        self.refund_storage(storage_start, false);
    }

    /**
    Execute executes given proposal. A proposal can be executed only once and only after the
    voting period passed and before the `proposal.execute_before`.
    Anyone can call this functions. */
    pub fn execute(&mut self, proposal_id: u32) -> Promise {
        let idx: u64 = proposal_id.into();
        let p = &mut self.proposals.get(idx).expect("proposal_id not found");
        let promise = p.execute(self.min_support);
        self.proposals.replace(idx, p);
        log!("Proposal {} executed.", proposal_id);
        return promise;
    }

    /// Returns proposal by id.
    /// Panics when `proposal_id` is not found.
    pub fn proposal(&self, proposal_id: u32) -> ProposalOut {
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
    use near_sdk::{testing_env, BlockHeight, MockedBlockchain};

    mod tutils;
    use crate::tests::tutils::deserialize_receipts;

    const BASE_UNIT: Balance = STORAGE_PRICE_PER_BYTE * 20;
    const DEFAULT_TRANSFER: Balance = 3000;

    fn setup_contract(min_support: u32) -> (VMContextBuilder, Contract) {
        let mut context = VMContextBuilder::new();
        testing_env!(context.build());
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
        let contract = Contract::new(voters, min_support, 10, 20, BASE_UNIT.into());
        testing_env!(context
            .predecessor_account_id(accounts(0))
            .attached_deposit(BASE_UNIT * 2)
            .build());
        (context, contract)
    }

    fn update_context(
        ctx: &mut VMContextBuilder,
        account: u8,
        deposit: Balance,
        block: BlockHeight,
    ) {
        testing_env!(ctx
            .predecessor_account_id(accounts(account.into()))
            .attached_deposit(deposit)
            .block_index(block)
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
        Contract::new(Vec::new(), 1000, 2, 2000, BASE_UNIT.into());
        Contract::new(Vec::new(), 10, 20, 21, BASE_UNIT.into());
    }

    fn setup_with_proposal() -> (VMContextBuilder, Contract, NewProposal) {
        let (mut ctx, mut contract) = setup_contract(5);
        // alice creates a proposal
        update_context(&mut ctx, 0, BASE_UNIT * 300, 1);
        let p = sample_proposal();
        contract.add_proposal(p.clone());
        return (ctx, contract, p);
    }

    #[test]
    fn test_happy_path() {
        let (mut ctx, mut contract, _p) = setup_with_proposal();
        // alice votes
        update_context(&mut ctx, 0, BASE_UNIT, 10);
        contract.vote(0, true);
        // bob votes
        update_context(&mut ctx, 1, BASE_UNIT, 11);
        contract.vote(0, false);
        // charlie votes
        update_context(&mut ctx, 2, BASE_UNIT, 12);
        contract.vote(0, true);

        update_context(&mut ctx, 2, BASE_UNIT, 21);
        let p = contract.proposal(0);
        assert_eq!(p.votes_for, 6);
        assert_eq!(p.votes_against, 3);
        assert_eq!(p.executed, false);
        assert_eq!(p.execute_before, 100.into());

        // anyone can execute a proposal, no need to attach any deposit.
        // must be between 31 and 100
        update_context(&mut ctx, 4, 0, 31);
        contract.execute(0);
        let p = contract.proposal(0);
        assert_eq!(p.executed, true);

        let receipts = deserialize_receipts();
        println!("Receipts: {:?}", receipts[0]);
        assert_eq!(receipts.len(), 1);
        assert_eq!(receipts[0].receiver_id, AccountId::from(accounts(3)));
        assert_eq!(receipts[0].actions.len(), 1);
        match &receipts[0].actions[0] {
            tutils::Action::Transfer(t) => assert_eq!(t.deposit, DEFAULT_TRANSFER),
            _ => panic!("invalid action type"),
        }
    }

    #[test]
    #[should_panic(expected = "proposal_id not found")]
    fn test_get_proposal() {
        let (mut ctx, mut contract) = setup_contract(5);
        // alice creates a proposal
        update_context(&mut ctx, 0, BASE_UNIT * 300, 1);
        let p_in = sample_proposal();
        let idx = contract.add_proposal(p_in.clone());
        assert_eq!(idx, 0);
        assert_eq!(contract.next_idx, 1);

        let p = contract.proposal(0);
        assert_eq!(
            p,
            ProposalOut {
                action: p_in.action,
                description: p_in.description,
                voting_start: p_in.voting_start,
                voting_end: 30.into(),
                votes_for: 0,
                votes_against: 0,
                execute_before: p.execute_before,
                executed: false
            }
        );

        // this panics
        contract.proposal(1);
    }

    #[test]
    #[should_panic(expected = "voting is not active")]
    fn test_vote_too_early() {
        let (mut ctx, mut contract, _p_in) = setup_with_proposal();
        // alice votes too early
        update_context(&mut ctx, 0, BASE_UNIT, 5);
        contract.vote(0, true);
    }

    #[test]
    #[should_panic(expected = "voting is not active")]
    fn test_vote_too_late() {
        let (mut ctx, mut contract, _p_in) = setup_with_proposal();
        // alice votes too late
        update_context(&mut ctx, 0, BASE_UNIT, 31);
        contract.vote(0, true);
    }

    #[test]
    #[should_panic(expected = "voting is not active")]
    fn test_vote_too_late2() {
        let (mut ctx, mut contract, _p_in) = setup_with_proposal();
        // alice votes too late - after execution period
        update_context(&mut ctx, 0, BASE_UNIT, 101);
        contract.vote(0, true);
    }

    #[test]
    #[should_panic(expected = "you (danny) are not authorized to vote")]
    fn test_vote_not_authorized() {
        let (mut ctx, mut contract, _p_in) = setup_with_proposal();
        // danny is not authorized to vote
        update_context(&mut ctx, 3, BASE_UNIT, 12);
        contract.vote(0, true);
    }

    #[test]
    #[should_panic(
        expected = "The required attached deposit is 90000000000000000000, but the given attached deposit is is 10000"
    )]
    fn test_vote_not_enough_deposit() {
        let (mut ctx, mut contract, _p_in) = setup_with_proposal();
        // alice didn't put enough deposit
        update_context(&mut ctx, 0, 10000, 12);
        contract.vote(0, true);
    }

    #[test]
    #[should_panic(expected = "proposal didn't get enough support (got 2, required: 5)")]
    fn test_execute_not_enough_support() {
        let (mut ctx, mut contract, _p) = setup_with_proposal();
        update_context(&mut ctx, 0, BASE_UNIT, 10);
        contract.vote(0, true);

        update_context(&mut ctx, 4, 0, 31);
        contract.execute(0);
    }

    #[test]
    #[should_panic(expected = "vote can be executed only between 31 and 100 block")]
    fn test_execute_too_early() {
        let (mut ctx, mut contract, _p) = setup_with_proposal();
        vote_alice_and_charile(&mut ctx, &mut contract);

        update_context(&mut ctx, 4, 0, 30);
        contract.execute(0);
    }

    #[test]
    #[should_panic(expected = "vote can be executed only between 31 and 100 block")]
    fn test_execute_too_late() {
        let (mut ctx, mut contract, _p) = setup_with_proposal();
        vote_alice_and_charile(&mut ctx, &mut contract);

        update_context(&mut ctx, 4, 0, 101);
        contract.execute(0);
    }

    #[test]
    #[should_panic(expected = "proposal already executed")]
    fn test_execute_twice() {
        let (mut ctx, mut contract, _p) = setup_with_proposal();
        vote_alice_and_charile(&mut ctx, &mut contract);

        update_context(&mut ctx, 4, 0, 40);
        contract.execute(0);
        update_context(&mut ctx, 4, 0, 50);
        contract.execute(0);
    }

    #[test]
    #[should_panic(expected = "proposal_id not found")]
    fn test_execute_not_found() {
        let (mut ctx, mut contract, _p) = setup_with_proposal();
        vote_alice_and_charile(&mut ctx, &mut contract);

        update_context(&mut ctx, 4, 0, 40);
        contract.execute(1);
    }

    #[test]
    fn test_execute_with_exact_support() {
        let (mut ctx, mut contract, _p) = setup_with_proposal();
        update_context(&mut ctx, 0, BASE_UNIT, 10);
        contract.vote(0, true);
        update_context(&mut ctx, 1, BASE_UNIT, 10); // together, alice and bob have power=5
        contract.vote(0, true);
        update_context(&mut ctx, 4, 0, 40);
        contract.execute(0);
        let p = contract.proposal(0);
        assert_eq!(p.executed, true);
    }

    fn vote_alice_and_charile(ctx: &mut VMContextBuilder, contract: &mut Contract) {
        update_context(ctx, 0, BASE_UNIT, 10);
        contract.vote(0, true);
        update_context(ctx, 2, BASE_UNIT, 10); // charile power = 5
        contract.vote(0, true);
    }

    fn sample_proposal() -> NewProposal {
        NewProposal {
            action: Action::Transfer {
                dest: accounts(3),
                amount: DEFAULT_TRANSFER.into(),
            },
            description: "transfer to danny".into(),
            voting_start: 10.into(),
            voting_duration: 20,
            execute_before: 100.into(),
        }
    }
}
