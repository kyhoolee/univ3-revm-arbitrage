use revm::db::Database;
use revm::primitives::{AccountInfo, Address, Bytecode, U256, B256};
use std::collections::HashMap;
use std::convert::Infallible;

/// InMemoryDB: full in-memory Database implement cho REVM.
#[derive(Debug, Clone, Default)]
pub struct InMemoryDB {
    pub accounts: HashMap<Address, AccountInfo>,
    pub storage: HashMap<(Address, U256), U256>,
}

impl InMemoryDB {
    /// Convert từ CacheDB<AlloyDB> → InMemoryDB
    pub fn from_cache_db<DB>(src: &revm::db::CacheDB<DB>) -> Self {
        let mut mem_db = InMemoryDB::default();

        // Clone account info
        for (addr, db_account) in &src.accounts {
            // Clone account info → AccountInfo luôn
            mem_db.accounts.insert(*addr, db_account.info.clone());

            // Clone storage slot
            for (slot, value) in &db_account.storage {
                mem_db.storage.insert((*addr, *slot), *value);
            }
        }

        mem_db
    }
}

/// Implement Database cho InMemoryDB
impl Database for InMemoryDB {
    type Error = Infallible;

    fn basic(&mut self, address: Address) -> Result<Option<AccountInfo>, Self::Error> {
        Ok(self.accounts.get(&address).cloned())
    }

    fn code_by_hash(&mut self, code_hash: B256) -> Result<Bytecode, Self::Error> {
        // Lookup code từ accounts
        for info in self.accounts.values() {
            if let Some(code) = &info.code {
                if code.hash_slow() == code_hash {
                    return Ok(code.clone());
                }
            }
        }
        Ok(Bytecode::new()) // Không tìm thấy → empty
    }

    fn storage(
        &mut self,
        address: Address,
        index: U256,
    ) -> Result<U256, Self::Error> {
        Ok(*self.storage.get(&(address, index)).unwrap_or(&U256::ZERO))
    }

    fn block_hash(&mut self, _number: u64) -> Result<B256, Self::Error> {
        Ok(B256::ZERO) // Fake luôn, không cần block hash
    }
}
