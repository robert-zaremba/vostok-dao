# NEAR Vostok DAO Guide

Vostok DAO is smart contract which creates a DAO to manage a $NEAR fund (wallet).

The best way to use the smart contract is to use the [near-cli](https://www.npmjs.com/package/near-cli). When using CLI, it's useful to define few variables which we are going to reuse in all calls:

```
CTR=wallet.myorg.guildnet  # contract address
NEAR_NODE="https://rpc.openshards.io"  # this is RPC endpoint. If using testnet you can use NODE_ENV=testnet instead
NEAR1="1_000000_000000_000000_000000";
NEAR0001="1_000_000000_000000_000000";  # 0.0001 NEAR
ALICE=alice.guildnet   # your account
```

In the examples we will use a guildnet (network managed by Openshards association).

The DAO functions by posting, voting and executing proposals. This is the standard flow:
* any member can post a proposal. When doing this, (s)he needs to put a bond (some amount of NEAR) to the proposal. When proposal passes or will be rejected, the bond is returned. We require a bond to avoid spam.
* Once proposal is there, any member can vote. Each member has assigned voting power.
* Each proposal has a voting time span, during which it is possible to vote to support a proposal.
* Once the voting time span is over and a proposal reached minimum amount of power votes then the proposal can be executed.

### Example

We have a vostok DAO with 3 voters: Alice, Bob and Charile, with respective power votes: 2, 3, 4. During the DAO setup, we set that each proposal has to reach minimum 5 power votes to pass.
Alice will create a proposal to send 10 NEAR from the DAO wallet to herself. The proposal will start now and last for 3 days. Alice and Bob will support (vote yes), a Charlie will oppose (vote no). At the end Alices' proposal will have 5 power votes support - which is exactly the minimum we set in the contract setup. Now, after 2 days, she can execute the proposal, which will send her 10 NEAR.
If Alice and Bob would vote not, and Charlie would vote yes, then the proposal won't pass, because it would have only 4 power votes proposals (from Charlie), but minimum required is 5.


### Setting up a contract

Firstly we need to decide about the DAO parameters:
* members (voters) - we can't change it later. If for some reason this will have to be changed, we should redeploy a nwe DAO. In v2 I'm will update this and add a function to allow signers change.
* voter power: each member has a voting power - any positive integer number.
* `min_support`: the minimum vote power a proposal has to receive to succeed.
* `min_duration`: each proposal must last at least that amount of time (in seconds).
* `max_duration`: each proposal must last at most that amount of time (in seconds).
* `min_bond`: the minimum amount of yocto NEAR a proposer has to put when creating a new proposal.


We will deploy a contract with `min_duration=10min` and `max_duration=3days`:

```
near --nodeUrl=$NEAR_NODE deploy --wasmFile ./res/vostok_dao.wasm --accountId $CTR  --initFunction "new" --initArgs '{"members": [{"account": "alice.guildnet", "power": 2}, {"account": "bob.guildnet", "power": 3}, {"account": "charlie.guildnet", "power": 4}], "min_support": 5, "min_duration": 600, "max_duration": 259200, "min_bond": "100000000000000000000000"}'
```

NOTE:
when using **mainnet** you should use `NODE_ENV=mainnet near ...` instead of `--nodeUrl=$NEAR_NODE`.

Let's inspect the contract settings:

```
near --nodeUrl=$NEAR_NODE view $CTR settings
```

### Creating a proposal

Alice creates a proposal to send 10 NEAR from the DAO:

```
near --nodeUrl=$NEAR_NODE --accountId $ALICE --amount 1 call $CTR add_proposal '{"p": {"action": {"Transfer": {"dest": "alice.guildnet", "amount": "10000000000000000000000000"}}, "description": "Send 10 NEAR to Alice", "voting_start": "1616275254", "voting_duration": 700, "execute_before": "1616276254"}}'
```

* `voting_start`: is the [Unix timestamp](https://www.unixtimestamp.com/) when the voting starts. NOTE: it's not the same as `env::block_timestamp()` which is measured in nanoseconds.
* `voting_duration`: time in seconds how long the voting will last. It must hold: `min_duration <= voting_duration <= max_duration`.
* `execute_before`: time (as Unix timestamp) **before** which the proposal (if passed) has to be executed. After that time the proposal is not valid anymore (even if it reached enough support).

Let's check our transaction status:

```
near --nodeUrl=$NEAR_NODE --accountId $CTR tx-status tx_id
```

### Vote

Now, each member can vote. Let's use Alice and  Bob account to vote for the proposal and Charlie to oppose:
```
near --nodeUrl=$NEAR_NODE --accountId alice.guildnet call $CTR vote '{"proposal_id": 0, "support": true}' --amount 0.0003
near --nodeUrl=$NEAR_NODE --accountId bob.guildnet call $CTR vote '{"proposal_id": 0, "support": true}' --amount 0.0003
near --nodeUrl=$NEAR_NODE --accountId charlie.guildnet call $CTR vote '{"proposal_id": 0, "support": false}' --amount 0.0003
```

* `--amount` is used to pay for the storage cost. Each vote is stored on a blockchain and each voter has to pay for it.

We can inspect the current status
```
near --nodeUrl=$NEAR_NODE view $CTR proposal '{"proposal_id": 0}'
```

### Execute

When a proposal passed and the current time is between `voting_end` and `execute_before` (use the `view proposal` call from the example above to see these values), anyone can make a transaction to execute the proposal. Since Alice made a proposal to withdraw to herself, most likely she will like to execute it. But for the example, we can use Charlie account:

```
near --nodeUrl=$NEAR_NODE --accountId charlie.guildnet call $CTR execute '{"proposal_id": 0}'
```

In the transaction, the DAO will send 10 NEAR to alice.
