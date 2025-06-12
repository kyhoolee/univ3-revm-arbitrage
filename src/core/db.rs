use alloy::{
    network::Ethereum,
    primitives::{keccak256, Address, Bytes, U256},
    providers::{Provider, RootProvider},
    sol_types::SolValue,
    transports::http::{Client, Http},
};
use anyhow::{anyhow, Result};
use revm::{
    db::{AlloyDB, CacheDB},
    primitives::{AccountInfo, Bytecode, ExecutionResult, Output, TransactTo, B256},
    Evm, EvmContext,
};
use std::sync::Arc;
use crate::core::provider::MultiProvider;
/// Wrapper để log các access đến storage
/// 
/// 
/// 

use revm::db::{Database, DatabaseCommit};

/// Wrapper quanh một Database để log các truy cập storage
pub struct LoggingDB<DB> {
    pub inner: DB,
}

impl<DB: Database> Database for LoggingDB<DB> {
    type Error = DB::Error;

    fn basic(&mut self, address: Address) -> Result<Option<AccountInfo>, Self::Error> {
        // Các phương thức khác chỉ cần gọi thẳng vào inner db
        self.inner.basic(address)
    }

    fn code_by_hash(&mut self, code_hash: B256) -> Result<Bytecode, Self::Error> {
        self.inner.code_by_hash(code_hash)
    }

    // Đây là phương thức chúng ta quan tâm
    fn storage(&mut self, address: Address, index: U256) -> Result<U256, Self::Error> {
        println!(
            "📦 DB Access:   Contract: {:?}, Slot: {:#x}",
            address, index
        );
        // Sau khi log, gọi phương thức của inner db
        self.inner.storage(address, index)
    }

    fn block_hash(&mut self, number: u64) -> Result<B256, Self::Error> {
        self.inner.block_hash(number)
    }
}



pub type AlloyCacheDB =
    CacheDB<AlloyDB<Http<Client>, Ethereum, Arc<RootProvider<Http<Client>>>>>;


use revm::db::{ EmptyDBTyped};
use std::convert::Infallible;

use crate::core::db_empty::InMemoryDB;

/// Convert CacheDB<AlloyDB> → CacheDB<EmptyDBTyped<Infallible>>
pub fn convert_cache_to_empty_db<DB>(src: &CacheDB<DB>) -> CacheDB<EmptyDBTyped<Infallible>> {
    let mut new_db = CacheDB::new(EmptyDBTyped::<Infallible>::default());

    // Clone account info + storage
    for (addr, db_account) in &src.accounts {
        // Clone account info
        new_db.insert_account_info(*addr, db_account.info.clone());

        // Clone storage
        for (slot, value) in &db_account.storage {
            new_db.insert_account_storage(*addr, *slot, *value).expect("insert_account_storage failed");
        }
    }

    new_db
}



pub fn init_cache_db_single(provider: Arc<RootProvider<Http<Client>>>) -> AlloyCacheDB {
    CacheDB::new(AlloyDB::new(provider, Default::default()).unwrap())
}

// Hàm init_cache_db của bạn
// Giờ provider.next() trả về Arc<ConcreteHttpProvider>,
// mà ConcreteHttpProvider là một kiểu Sized và implements Provider
// Nên AlloyDB::new có thể chấp nhận nó tùy thuộc vào signature của nó.
pub fn init_cache_db(multi_provider: &MultiProvider) -> AlloyCacheDB {
    let (provider, url) = multi_provider.next();
    // Vẫn cần kiểm tra lại signature của AlloyDB::new
    // Nếu nó cần T: Provider + Sized, thì Arc<ConcreteHttpProvider> là phù hợp.
    // Nếu nó cần Arc<T: Provider>, thì Arc<ConcreteHttpProvider> cũng phù hợp.
    CacheDB::new(AlloyDB::new(provider, Default::default()).unwrap())
}

// ... các import và định nghĩa struct/impl khác cho CacheDB, AlloyDB ...

// pub async fn init_account(
//     address: Address,
//     cache_db: &mut AlloyCacheDB,
//     provider: Arc<RootProvider<Http<Client>>>,
// ) -> Result<()> {
//     let cache_key = format!("bytecode-{:?}", address);
//     let bytecode = match cacache::read(&cache_dir(), cache_key.clone()).await {
//         Ok(bytecode) => {
//             let bytecode = Bytes::from(bytecode);
//             Bytecode::new_raw(bytecode)
//         }
//         Err(_) => {
//             let bytecode = provider.get_code_at(address).await?;
//             let bytecode_result = Bytecode::new_raw(bytecode.clone());
//             let bytecode_vec = bytecode.to_vec();
//             cacache::write(&cache_dir(), cache_key, bytecode_vec).await?;
//             bytecode_result
//         }
//     };
//     let code_hash = bytecode.hash_slow();
//     let acc_info = AccountInfo {
//         balance: U256::ZERO,
//         nonce: 0_u64,
//         code: Some(bytecode),
//         code_hash,
//     };
//     cache_db.insert_account_info(address, acc_info);
//     Ok(())
// }

pub async fn init_account(
    address: Address,
    cache_db: &mut AlloyCacheDB,
    multi_provider: &MultiProvider,
) -> Result<()> {
    use crate::core::logger::measure_start;

    let cache_key = format!("bytecode-{:?}", address);

    let start = measure_start(&format!("init_account {:?}", address));

    let (provider, url) = multi_provider.next();  // lấy (provider, url)

    println!("Init account {:?} using RPC {}", address, url);

    let bytecode = match cacache::read(&cache_dir(), cache_key.clone()).await {
        Ok(bytecode) => {
            let bytecode = Bytes::from(bytecode);
            Bytecode::new_raw(bytecode)
        }
        Err(_) => {
            let bytecode = provider.get_code_at(address).await?;
            let bytecode_result = Bytecode::new_raw(bytecode.clone());
            let bytecode_vec = bytecode.to_vec();
            cacache::write(&cache_dir(), cache_key, bytecode_vec).await?;
            bytecode_result
        }
    };

    let code_hash = bytecode.hash_slow();
    let acc_info = AccountInfo {
        balance: U256::ZERO,
        nonce: 0_u64,
        code: Some(bytecode),
        code_hash,
    };
    cache_db.insert_account_info(address, acc_info);

    crate::core::logger::measure_end(start);

    Ok(())
}


pub fn init_account_with_bytecode(
    address: Address,
    bytecode: Bytecode,
    cache_db: &mut AlloyCacheDB,
) -> Result<()> {
    let code_hash = bytecode.hash_slow();
    let acc_info = AccountInfo {
        balance: U256::ZERO,
        nonce: 0_u64,
        code: Some(bytecode),
        code_hash,
    };
    cache_db.insert_account_info(address, acc_info);
    Ok(())
}

pub fn insert_mapping_storage_slot(
    contract: Address,
    slot: U256,
    slot_address: Address,
    value: U256,
    cache_db: &mut AlloyCacheDB,
) -> Result<()> {
    let hashed_slot = keccak256((slot_address, slot).abi_encode());
    cache_db.insert_account_storage(contract, hashed_slot.into(), value)?;
    Ok(())
}

pub fn revm_call(
    from: Address,
    to: Address,
    calldata: Bytes,
    cache_db: &mut AlloyCacheDB,
) -> Result<Bytes> {
    // Khởi tạo inspector của bạn
    // let mut inspector = StorageLoggerInspector::default();
    let logging_db = LoggingDB { inner: cache_db };
    let mut evm = Evm::builder()
        .with_db(logging_db)
        
        .modify_tx_env(|tx| {
            tx.caller = from;
            tx.transact_to = TransactTo::Call(to);
            tx.data = calldata;
            tx.value = U256::ZERO;
        })
        .build();

    // evm.set_inspector(inspector);

    let result = evm.transact()?.result;

    let value = match result {
        ExecutionResult::Success {
            output: Output::Call(value),
            ..
        } => value,
        result => {
            return Err(anyhow!("execution failed: {result:?}"));
        }
    };

    Ok(value)
}



pub fn revm_call_db(
    from: Address,
    to: Address,
    calldata: Bytes,
    cache_db: &mut InMemoryDB,
) -> Result<Bytes> {
    let mut evm = Evm::builder()
        .with_db(cache_db)
        .modify_tx_env(|tx| {
            tx.caller = from;
            tx.transact_to = TransactTo::Call(to);
            tx.data = calldata;
            tx.value = U256::ZERO;
        })
        .build();

    let result = evm.transact()?.result;

    let value = match result {
        ExecutionResult::Success {
            output: Output::Call(value),
            ..
        } => value,
        result => {
            return Err(anyhow!("execution failed: {result:?}"));
        }
    };

    Ok(value)
}



pub fn revm_revert(
    from: Address,
    to: Address,
    calldata: Bytes,
    cache_db: &mut AlloyCacheDB,
) -> Result<Bytes> {
    let mut evm = Evm::builder()
        .with_db(cache_db)
        .modify_tx_env(|tx| {
            tx.caller = from;
            tx.transact_to = TransactTo::Call(to);
            tx.data = calldata;
            tx.value = U256::ZERO;
        })
        .build();

    let result = evm.transact()?.result;

    match result {
        ExecutionResult::Revert { output, .. } => Ok(output),
        _ => Err(anyhow!("Expected revert result")),
    }
}

fn cache_dir() -> String {
    ".evm_cache".to_string()
}
