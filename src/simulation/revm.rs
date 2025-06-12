use crate::simulation::{state_db::StateDB, SimulationStrategyTrait};
use alloy_eips::{BlockId, BlockNumberOrTag};
use alloy_primitives::{address, aliases::U112, Address, Bytes, TxKind, B256, U256};
use alloy_provider::{network::Ethereum, ProviderBuilder, RootProvider};
use alloy_sol_types::SolValue;
use alloy_transport_http::Http;
use anyhow::{Error, Result};
use e_primitives::structs::{pool::UniV2Data, Pool, State, Transaction};
use reqwest::Client;
use revm::{
    db::AlloyDB,
    primitives::{keccak256, AccountInfo, BlobExcessGasAndPrice, BlockEnv, ExecutionResult, ResultAndState, TxEnv},
    DatabaseRef, Evm,
};
use revm_primitives::Log;
use std::{collections::HashMap, str::FromStr, sync::Arc};

use super::UNISWAP_V2_TOPIC;

type StateCacheDB = StateDB<AlloyDB<Http<Client>, Ethereum, Arc<RootProvider<Http<Client>>>>>;

pub struct Revm {
    pub state: State,
    db: Option<StateCacheDB>,
    pub block: BlockEnv,
    pub rpc_url: String,
}

pub struct RTransaction(Transaction);
impl From<RTransaction> for TxEnv {
    fn from(tx: RTransaction) -> Self {
        let tx = tx.0;
        TxEnv {
            caller: Address::from_str(tx.from.as_str()).unwrap(),
            gas_limit: tx.gas,
            gas_price: U256::from(tx.gas_price),
            transact_to: TxKind::Call(Address::from_str(tx.to.as_str()).unwrap()),
            value: U256::from(tx.value),
            data: Bytes::from_str(tx.input.as_str()).unwrap(),
            nonce: tx.nonce,
            chain_id: Some(1),
            access_list: vec![],
            gas_priority_fee: Some(U256::from(tx.max_priority_fee_per_gas)),
            blob_hashes: vec![],
            max_fee_per_blob_gas: None,
            authorization_list: None,
        }
    }
}

impl Revm {
    pub fn new(rpc_url: &str, block_id: BlockId, block: BlockEnv) -> Self {
        Self {
            state: State::default(),
            rpc_url: rpc_url.to_string(),
            db: Revm::build_db(rpc_url, block_id, None).ok(),
            block,
        }
    }

    pub fn new_with_state(rpc_url: &str, block: BlockId, state: State) -> Self {
        let pools = state.pools.clone();
        let block_env = BlockEnv {
            number: U256::from(state.block.number),
            difficulty: U256::from(state.block.difficulty),
            prevrandao: Some(B256::ZERO),
            gas_limit: U256::from(state.block.gas_limit),
            basefee: U256::from(state.block.basefee),
            blob_excess_gas_and_price: Some(BlobExcessGasAndPrice::new(0)),
            coinbase: address!("4838b106fce9647bdf1e7877bf73ce8b0bad5f97"),
            timestamp: U256::from(state.block.timestamp),
        };
        tracing::debug!("Creating revm with block: {block}, pools size: {}", state.pools.len());
        let remv_instance = Self {
            state,
            db: Revm::build_db(rpc_url, block, Some(pools)).ok(),
            block: block_env,
            rpc_url: rpc_url.to_string(),
        };
        tracing::debug!("Revm instance created");
        remv_instance
    }

    fn build_db(rpc_url: &str, block: BlockId, pools: Option<HashMap<String, Pool>>) -> Result<StateCacheDB> {
        let client = ProviderBuilder::new().on_http(rpc_url.parse()?);
        let client = Arc::new(client);
        let state_db = AlloyDB::new(client, block).expect("Failed to create AlloyDB");
        let cache_db: StateCacheDB = match pools {
            Some(pools) => StateDB::new_with_state(state_db, pools),
            None => StateDB::new(state_db),
        };
        Ok(cache_db)
    }

    pub fn clone(&self, sync_originals: bool) -> Result<Self> {
        let client = ProviderBuilder::new().on_http(self.rpc_url.parse()?);
        let client = Arc::new(client);
        let block = BlockId::Number(BlockNumberOrTag::Number(self.state.block.number));
        let alloy_db = AlloyDB::new(client, block).expect("Failed to create AlloyDB");
        let mut state_db = self.db.as_ref().unwrap().clone(alloy_db);
        if sync_originals {
            state_db.sync_originals();
        }
        return Ok(Self {
            state: self.state.clone(),
            db: Some(state_db),
            block: self.block.clone(),
            rpc_url: self.rpc_url.clone(),
        });
    }

    fn handle_logs(&mut self, logs: &Vec<Log>) -> Result<()> {
        for log in logs {
            if let Some(topic) = log.data.topics().first() {
                if topic.eq(UNISWAP_V2_TOPIC) {
                    let data = log.data.data.clone();
                    self.handle_topic_v2(&log.address, &data)?;
                }
            }
        }
        Ok(())
    }

    fn handle_topic_v2(&mut self, address: &Address, data: &Bytes) -> Result<()> {
        let (rs0, rs1) = <(U112, U112)>::abi_decode(data, false)?;
        let pool = self.state.pools.get_mut(&address.to_string().to_lowercase());

        if let Some(pool) = pool {
            pool.v2_data = Some(UniV2Data::new(U256::from(rs0), U256::from(rs1)));
        }

        Ok(())
    }

    pub fn create_account(&mut self, account: Address, balance_eth: U256) -> Result<()> {
        let db = self.db.as_mut().ok_or_else(|| Error::msg("DB is not initialized"))?;
        let acc_info = AccountInfo {
            nonce: 0_u64,
            balance: balance_eth,
            code_hash: keccak256(Bytes::new()),
            code: None,
        };

        db.insert_account_info(account, acc_info);
        Ok(())
    }

    pub fn get_storage(&self, addr: Address, idx: U256) -> Result<U256> {
        let db = self.db.as_ref().ok_or_else(|| Error::msg("DB is not initialized"))?;
        let data = db.storage_ref(addr, idx)?;
        Ok(data)
    }

    pub fn set_token_balance(&mut self, account: Address, token: Address, slot: U256, balance: U256) -> Result<()> {
        let db = self.db.as_mut().ok_or_else(|| Error::msg("DB is not initialized"))?;
        let hashed_acc_balance_slot = keccak256((account, slot).abi_encode());
        let _ = db.insert_account_storage(token, hashed_acc_balance_slot.into(), balance);
        Ok(())
    }

    /// Experimental function may be removed in the future
    pub fn transact_no_block_commit(&mut self, tx: &Transaction) -> Result<ExecutionResult, Error> {
        let db = self.db.as_mut().ok_or_else(|| Error::msg("DB is not initialized"))?;
        let mut evm = Evm::builder()
            .with_db(db)
            .modify_tx_env(|etx| {
                let revm_tx = RTransaction(tx.clone());
                *etx = revm_tx.into();
            })
            .build();
        let result = evm.transact_commit().map_err(|e| Error::msg(e.to_string()))?;
        drop(evm);

        if let ExecutionResult::Success { logs, .. } = &result {
            let _ = self.handle_logs(logs)?;
        }

        Ok(result)
    }

    /// Experimental function may be removed in the future
    pub fn transact_no_block(&mut self, tx: &Transaction) -> Result<ResultAndState, Error> {
        let db = self.db.as_mut().ok_or_else(|| Error::msg("DB is not initialized"))?;
        let mut evm = Evm::builder()
            .with_db(db)
            .modify_tx_env(|etx| {
                let revm_tx = RTransaction(tx.clone());
                *etx = revm_tx.into();
            })
            .build();
        let result = evm.transact().map_err(|e| Error::msg(e.to_string()))?;
        Ok(result)
    }
}

impl SimulationStrategyTrait for Revm {
    fn transact(&mut self, tx: &Transaction) -> Result<ResultAndState, Error> {
        let db = self.db.as_mut().ok_or_else(|| Error::msg("DB is not initialized"))?;
        let mut evm = Evm::builder()
            .with_db(db)
            .modify_block_env(|b| {
                *b = self.block.clone();
            })
            .modify_tx_env(|etx| {
                let revm_tx = RTransaction(tx.clone());
                *etx = revm_tx.into();
            })
            .build();
        let result = evm.transact().map_err(|e| Error::msg(e.to_string()))?;
        Ok(result)
    }

    fn transact_commit(&mut self, tx: &Transaction) -> Result<ExecutionResult, Error> {
        tracing::debug!("Executing tx {}", tx.hash.to_string());
        let db = self.db.as_mut().ok_or_else(|| Error::msg("DB is not initialized"))?;
        let mut evm = Evm::builder()
            .with_db(db)
            .modify_block_env(|b| {
                *b = self.block.clone();
            })
            .modify_tx_env(|etx| {
                let revm_tx = RTransaction(tx.clone());
                *etx = revm_tx.into();
            })
            .build();
        let result = evm.transact_commit().map_err(|e| Error::msg(e.to_string()))?;
        tracing::debug!("Executed tx {}", tx.hash.to_string());
        drop(evm);
        if let ExecutionResult::Success { logs, .. } = &result {
            let _ = self.handle_logs(logs)?;
        }

        Ok(result)
    }

    fn get_state(&self) -> State {
        self.state.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::finder::FROM_CALL;
    use alloy_eips::BlockNumberOrTag;
    use alloy_sol_types::SolValue;
    use e_primitives::{
        enums::PoolProtocol,
        structs::{
            pool::{Token, UniV2Data},
            BlockInfo,
        },
    };
    use lazy_static::lazy_static;
    use revm::primitives::{Output, SuccessReason};
    use std::env;

    lazy_static! {
        static ref RPC_URL: String = env::var("RPC_URL").expect("Please set RPC_URL env");
    }

    fn build_state() -> State {
        let mut state = State::default();
        state.block = BlockInfo {
            number: 21082624,
            basefee: 8881116053,
            gas_limit: 30000000,
            difficulty: 0,
            timestamp: 1730341571,
        };
        state
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_revm_with_state() {
        let mut state = build_state();
        let pool_addr = "0x9a84a1852bc7fb608794960960adb04666a12b41".to_string();
        let token0 = "0x50327c6c5a14DCaDE707ABad2E27eB517df87AB5".to_string();
        state.pools.insert(
            pool_addr.clone(),
            Pool {
                protocol: PoolProtocol::UniSwapV2,
                address: pool_addr.clone(),
                v2_data: Some(UniV2Data::new(
                    U256::from(1422753635285u64),
                    U256::from(247356960913u64),
                )),
                token0: Token { address: token0.clone(), slot: 1 },
                ..Default::default()
            },
        );
        let mut revm = Revm::new_with_state(
            &RPC_URL.clone().as_str(),
            BlockId::Number(BlockNumberOrTag::Number(21082623)),
            state,
        );

        let result = revm.transact_no_block(&Transaction {
            block_number: 21082624,
            input: "0x0902f1ac".to_string(),
            from: FROM_CALL.to_string(),
            to: pool_addr.clone(),
            value: 0,
            ..Default::default()
        });
        assert!(result.is_ok());
        let ResultAndState { result, .. } = result.unwrap();
        match result {
            ExecutionResult::Success { output: Output::Call(data), .. } => {
                let (reserve0, reserve1, _) = <(U256, U256, u32)>::abi_decode(&data, false).unwrap();
                assert_eq!(reserve0, U256::from(1422753635285u64));
                assert_eq!(reserve1, U256::from(247356960913u64));
            },
            _ => assert!(false, "Expected revert"),
        }

        let result = revm.transact_no_block(&Transaction {
            block_number: 21082624,
            input: "0x0dfe1681".to_string(),
            from: FROM_CALL.to_string(),
            to: pool_addr,
            value: 0,
            ..Default::default()
        });
        assert!(result.is_ok());
        let ResultAndState { result, .. } = result.unwrap();
        match result {
            ExecutionResult::Success { output: Output::Call(data), .. } => {
                let token0_decoded = U256::abi_decode(&data, false).unwrap();
                assert_eq!(token0_decoded, U256::from_str_radix(&token0[2..], 16).unwrap());
            },
            _ => assert!(false, "Expected revert"),
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_revm_transact_commit_success() {
        let mut state = build_state();
        state.pools.insert(
            "0x9a84a1852bc7fb608794960960adb04666a12b41".to_string(),
            Pool {
                protocol: PoolProtocol::UniSwapV2,
                v2_data: Some(UniV2Data::new(
                    U256::from(1437419923157u64),
                    U256::from(244825610915u64),
                )),
                ..Default::default()
            },
        );

        let mut revm = Revm::new_with_state(
            &RPC_URL.clone().as_str(),
            BlockId::Number(BlockNumberOrTag::Number(21082623)),
            state,
        );

        let tx = Transaction {
            hash: "0x67f53fa68781ef3b3d40a55fbdf585a0f240913a2f5cf09c1258259f6d61ed08".to_string(),
            block_number: 21082624,
            input: "0x5f5755290000000000000000000000000000000000000000000000000000000000000080000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000014d1120d7b16000000000000000000000000000000000000000000000000000000000000000000c000000000000000000000000000000000000000000000000000000000000000136f6e65496e6368563546656544796e616d6963000000000000000000000000000000000000000000000000000000000000000000000000000000000000000400000000000000000000000000000000000000000000000000000000000000000000000000000000000000000050327c6c5a14dcade707abad2e27eb517df87ab500000000000000000000000000000000000000000000000014a270ef4868b000000000000000000000000000000000000000000000000000000000052ea125810000000000000000000000000000000000000000000000000000000000000120000000000000000000000000000000000000000000000000002ea11e32ad5000000000000000000000000000f326e4de8f66a0bdc0970b79e0924e33c79f1915000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000002c812aa3caf0000000000000000000000003451b6b219478037a1ac572706627fc2bda1e812000000000000000000000000eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee00000000000000000000000050327c6c5a14dcade707abad2e27eb517df87ab50000000000000000000000003451b6b219478037a1ac572706627fc2bda1e81200000000000000000000000074de5d4fcbf63e00296fd95d33236b979401663100000000000000000000000000000000000000000000000014a270ef4868b000000000000000000000000000000000000000000000000000000000052ea125810000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000014000000000000000000000000000000000000000000000000000000000000001600000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000012600000000000000000000000000000000000000000000000000000000010800a007e5c0d20000000000000000000000000000000000000000000000e400007d00001a4041c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2d0e30db002a000000000000000000000000000000000000000000000000000000000e64611beee63c1e580e0554a476a092703abdb3ef35c80e0d76d32939fc02aaa39b223fe8d0a0e5c4f27ead9083c756cc29a84a1852bc7fb608794960960adb04666a12b4100206ae4071138002dc6c09a84a1852bc7fb608794960960adb04666a12b411111111254eeb25477b68fb85ed929f73a960582000000000000000000000000000000000000000000000000000000052ea12581a0b86991c6218b36c1d19d4a2e9eb0ce3606eb4800000000000000000000000000000000000000000000000000007dcbea7c00000000000000000000000000000000000000000000000001a7".to_string(),
            input_decoded: None,
            to: "0x881d40237659c251811cec9c364ef91dc08d300c".to_string(),
            from: "0x8d309e350d7700c1f8ca3a72c95018f5859182d9".to_string(),
            gas: 338795,
            nonce: Some(124),
            max_priority_fee_per_gas: 2000000000,
            gas_price: 10881116053,
            index: 0,
            value: 1500000000000000000,
        };
        let result = revm.transact_commit(&tx);
        assert!(result.is_ok());
        let result = result.unwrap();
        match result {
            ExecutionResult::Success { logs, reason, gas_refunded, output, .. } => {
                assert_eq!(logs.len(), 11);
                assert_eq!(reason, SuccessReason::Stop);
                assert_eq!(gas_refunded, 31100);
                assert_eq!(output, Output::Call(Bytes::new()));
            },
            _ => assert!(false, "Expected success"),
        }

        let state = revm.get_state();
        assert_eq!(state.pools.len(), 1);
        let v2_data = state
            .pools
            .get("0x9a84a1852bc7fb608794960960adb04666a12b41")
            .unwrap()
            .v2_data
            .clone()
            .unwrap();
        assert_eq!(v2_data.reserve_0, U256::from(1414704508972u64));
        assert_eq!(v2_data.reserve_1, U256::from(248768518616u64));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_revm_transact_commit_revert() {
        let state = build_state();
        let mut revm = Revm::new_with_state(
            &RPC_URL.clone().as_str(),
            BlockId::Number(BlockNumberOrTag::Number(21082623)),
            state,
        );
        let tx = Transaction {
            hash: "0x67f53fa68781ef3b3d40a55fbdf585a0f240913a2f5cf09c1258259f6d61ed07".to_string(),
            block_number: 21082624,
            input: "0x0902f1ac".to_string(),
            input_decoded: None,
            to: "0x881d40237659c251811cec9c364ef91dc08d300c".to_string(),
            from: "0000000000000000000000000000000000000001".to_string(),
            gas: 338795,
            nonce: Some(0),
            max_priority_fee_per_gas: 2000000000,
            gas_price: 10881116053,
            index: 0,
            value: 1500000000000000000,
        };

        let result = revm.transact_commit(&tx);
        assert!(result.is_ok());
        let result = result.unwrap();
        match result {
            ExecutionResult::Revert { gas_used, output } => {
                assert_eq!(gas_used, 21234);
                assert_eq!(output, Bytes::new());
            },
            _ => assert!(false, "Expected revert"),
        }

        let state = revm.get_state();
        assert!(state.pools.is_empty());
    }

    // #[tokio::test(flavor = "multi_thread")]
    // async fn test_revm_transact_commit_halt() {
    //     let state = build_state();
    //     let mut revm = Revm::new_with_state(state);
    //     revm.build_db(RPC_URL, BlockId::Number(BlockNumberOrTag::Number(21082623)))
    //         .await
    //         .unwrap();
    //
    //     let tx = Transaction {
    //         hash: "0xe45832b9cf3c8d692f2c15c956300c3a85a1e700f3222900ecfa9644be092291".to_string(),
    //         block_number: 21082624,
    //         input: "0xcdebe0c600000000000000000000000000000000000000000000000000000000005b8d800000000000000000000000000000000000000000000000000000000000002ddf00000000000000000000000000000000000000000000000000000000000000800000000000000000000000000000000000000000000000000000000000000420000000000000000000000000000000000000000000000000000000000000036800000185d5c9254000000000000000000000000000000000000000000000000000000000007a120000000000000000000000000000000000000000000000000000000000000000cd000000000000000000000000201bfae52b060b9e835f5aac0c72063fa58a5a99000000000000000000000000a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48000000000000000000000000000000000000000000000000000000000000008900000000000000000000000000000000000000000000000000000000000000e000000000000000000000000000000000000000000000000000000000000002200000000000000000000000007e7a0e201fd38d3adaa9523da6c109a07118c96a0000000000000000000000001b84765de8b7566e4ceaf4d0fd3c5af52d3dde4f0000000000000000000000000000000000000000000000006eaa5f8dd57600ed000000000000000000000000000000000000000000000000000000006722f07000000000000000000000000000000000000000000000000000000000000000a0000000000000000000000000000000000000000000000000000000000000008000000000000000000000000000000000000000000000000000000000000000010000000000000000000000001116898dda4015ed8ddefb84b6e8bc24528af2d8000000000000000000000000000000000000000000000000000000000000000100000000000000000000000000000000000000000000000000000000000000ff0000000000000000000000007e7a0e201fd38d3adaa9523da6c109a07118c96a0000000000000000000000002791bca1f2de4661ed88a30c99a7a9449aa84174000000000000000000000000000000000000000000000000000000000071aba300000000000000000000000000000000000000000000000000000000672c24b000000000000000000000000000000000000000000000000000000000000000a00000000000000000000000000000000000000000000000000000000000000080000000000000000000000000000000000000000000000000000000000000000000000000000000000000000085fcd7dd0a1e1a9fcd5fd886ed522de8221c3ee5000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000041e7c78eb65cebe3e048fcd81b834671db5e49bbc4741c1b3ef268b2a39684a8a12ccf8435565e99c212c4012559d290dc683cf106f98737d9a00da78b6db143211c00000000000000000000000000000000000000000000000000000000000000".to_string(),
    //         input_decoded: None,
    //         to: "0x07042134d4dc295cbf3ab08a4a0eff847a528171".to_string(),
    //         from: "0x78246ac69cce0d90a366b2d52064a88bb4ad8467".to_string(),
    //         gas: 500000,
    //         nonce: 15631,
    //         max_priority_fee_per_gas: 10117702505,
    //         gas_price: 10117702505,
    //         index: 97,
    //         value: 0,
    //     };
    //
    //     let result = revm.transact_commit(&tx);
    //     assert!(result.is_ok());
    //     let result = result.unwrap();
    //     match result {
    //         ExecutionResult::Halt { reason, gas_used } => {
    //             assert_eq!(gas_used, 0);
    //             assert_eq!(HaltReason::CallTooDeep, reason);
    //         },
    //         _ => panic!("Expected success"),
    //     }
    // }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_revm_transact_success() {
        let state = build_state();
        let mut revm = Revm::new_with_state(
            &RPC_URL.clone().as_str(),
            BlockId::Number(BlockNumberOrTag::Number(21082624)),
            state,
        );

        let tx = Transaction {
            hash: "0x0000000000000000000000000000000000000001".to_string(),
            block_number: 21082624,
            input: "0x0902f1ac".to_string(),
            input_decoded: None,
            to: "0x9a84a1852bc7fb608794960960adb04666a12b41".to_string(),
            from: "0x0000000000000000000000000000000000000001".to_string(),
            gas: 500000,
            nonce: Some(0),
            max_priority_fee_per_gas: 0,
            gas_price: 8881116053,
            index: 97,
            value: 0,
        };

        let result = revm.transact(&tx);
        assert!(result.is_ok());
        let ResultAndState { result, .. } = result.unwrap();
        match result {
            ExecutionResult::Success { output, .. } => {
                assert_eq!(output, Output::Call(Bytes::from_str("0x000000000000000000000000000000000000000000000000000001497e46294b00000000000000000000000000000000000000000000000000000039e7ddd519000000000000000000000000000000000000000000000000000000006722eac3").unwrap()));
            },
            _ => assert!(false, "expect success"),
        }
        let state = revm.get_state();
        let expected_state = State::default();
        assert_eq!(state.pools.len(), expected_state.pools.len());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_revm_transact_revert() {
        let state = build_state();
        let mut revm = Revm::new_with_state(
            &RPC_URL.clone().as_str(),
            BlockId::Number(BlockNumberOrTag::Number(21082624)),
            state,
        );

        let tx = Transaction {
            hash: "0x0000000000000000000000000000000000000002".to_string(),
            block_number: 21082624,
            input: "0x0902f1ad".to_string(),
            input_decoded: None,
            to: "0x9a84a1852bc7fb608794960960adb04666a12b41".to_string(),
            from: "0x0000000000000000000000000000000000000001".to_string(),
            gas: 500000,
            nonce: Some(0),
            max_priority_fee_per_gas: 0,
            gas_price: 8881116053,
            index: 97,
            value: 0,
        };

        let result = revm.transact(&tx);
        assert!(result.is_ok());
        let ResultAndState { result, .. } = result.unwrap();
        match result {
            ExecutionResult::Revert { .. } => {
                assert!(true);
            },
            _ => assert!(false, "expect revert"),
        }
    }

    // #[tokio::test(flavor = "multi_thread")]
    // async fn test_revm_transact_halt() {
    //     let mut revm = Revm::new();
    //     let result = revm.transact(&Transaction::default());
    //     assert!(result.is_ok());
    //     let ResultAndState { result, state } = result.unwrap();
    //     let state = revm.get_state();
    //     let expected_state = State::default();
    //     assert_eq!(state.pools.len(), expected_state.pools.len());
    // }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_revm_transact_with_state_db_clone() {
        let mut state = build_state();
        state.pools.insert(
            "0x9a84a1852bc7fb608794960960adb04666a12b41".to_string(),
            Pool {
                protocol: PoolProtocol::UniSwapV2,
                address: "0x9a84a1852bc7fb608794960960adb04666a12b41".to_string(),
                v2_data: Some(UniV2Data {
                    reserve_0: U256::from(23),
                    reserve_1: U256::from(12),
                }),
                ..Default::default()
            },
        );
        let mut revm = Revm::new_with_state(
            &RPC_URL.clone().as_str(),
            BlockId::Number(BlockNumberOrTag::Number(21082624)),
            state,
        );
        let tx = Transaction {
            hash: "0x0000000000000000000000000000000000000005".to_string(),
            block_number: 21082624,
            input: "0x0902f1ac".to_string(),
            input_decoded: None,
            to: "0x9a84a1852bc7fb608794960960adb04666a12b41".to_string(),
            from: "0x0000000000000000000000000000000000000001".to_string(),
            gas: 500000,
            nonce: Some(0),
            max_priority_fee_per_gas: 0,
            gas_price: 8881116053,
            index: 97,
            value: 0,
        };
        let result = revm.transact(&tx);
        assert!(result.is_ok());
        let ResultAndState { result, .. } = result.unwrap();
        match result {
            ExecutionResult::Success { output: Output::Call(value), .. } => {
                let (reserve0, reserve1, _) = <(U256, U256, u32)>::abi_decode(&value, false).unwrap();
                assert_eq!(reserve0, U256::from(23));
                assert_eq!(reserve1, U256::from(12));
            },
            _ => assert!(false, "expect success"),
        }

        let state = revm.get_state();
        assert_eq!(state.pools.len(), 1);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_set_token_balance() {
        let state = build_state();
        let mut revm = Revm::new_with_state(
            &RPC_URL.clone().as_str(),
            BlockId::Number(BlockNumberOrTag::Number(21082624)),
            state,
        );
        let account = address!("171EA1194533286ECD1B693e4Af2873A6264f690");
        let balance = U256::from(100);
        let usdt = address!("dac17f958d2ee523a2206206994597c13d831ec7");
        let result = revm.set_token_balance(account, usdt, U256::from(2), balance);
        assert!(result.is_ok());

        let tx = Transaction {
            hash: "0x67f53fa68781ef3b3d40a55fbdf585a0f240913a2f5cf09c1258259f6d61ed08".to_string(),
            block_number: 21082624,
            input: "0x27e235e3000000000000000000000000171ea1194533286ecd1b693e4af2873a6264f690".to_string(),
            from: "0x0000000000000000000000000000000000000000".to_string(),
            to: "0xdac17f958d2ee523a2206206994597c13d831ec7".to_string(),
            ..Default::default()
        };
        let result = revm.transact_no_block(&tx);
        assert!(result.is_ok());
        let result = result.unwrap();
        match result {
            ResultAndState {
                result: ExecutionResult::Success { output: Output::Call(value), .. },
                ..
            } => {
                let balance_decoded = <U256>::abi_decode(&value, false).unwrap();
                assert_eq!(balance, balance_decoded);
            },
            _ => assert!(false, "Expected success"),
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_create_account() {
        let state = build_state();
        let mut revm = Revm::new_with_state(
            &RPC_URL.clone().as_str(),
            BlockId::Number(BlockNumberOrTag::Number(21082624)),
            state,
        );
        let account = address!("171EA1194533286ECD1B693e4Af2873A6264f690");
        let balance_eth = U256::from(2312);
        let result = revm.create_account(account, balance_eth);
        assert!(result.is_ok());
        let tx = Transaction {
            hash: "0x67f53fa68781ef3b3d40a55fbdf585a0f240913a2f5cf09c1258259f6d61ed08".to_string(),
            block_number: 21082624,
            from: "0x0000000000000000000000000000000000000000".to_string(),
            to: "171EA1194533286ECD1B693e4Af2873A6264f690".to_string(),
            ..Default::default()
        };
        let result = revm.transact_no_block(&tx);
        assert!(result.is_ok());
        let result = result.unwrap();
        match result {
            ResultAndState {
                result: ExecutionResult::Success { .. },
                state,
                ..
            } => {
                let balance_eth_state = state.get(&account).unwrap().info.balance;
                assert_eq!(balance_eth, balance_eth_state);
            },
            _ => assert!(false, "Expected success"),
        }
    }
}
