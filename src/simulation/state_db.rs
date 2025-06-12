use e_primitives::{enums::PoolProtocol, structs::Pool};
use revm::{
    primitives::{Account, AccountInfo, Address, Bytecode, HashMap, B256, KECCAK_EMPTY, U256},
    Database, DatabaseCommit, DatabaseRef,
};
use std::{
    collections::{hash_map::Entry, HashMap as StdHashMap},
    str::FromStr,
};

/// A [Database] implementation that stores all state changes in memory.
///
/// This implementation wraps a [DatabaseRef] that is used to load data ([AccountInfo]).
///
/// Accounts and code are stored in two separate maps, the `accounts` map maps addresses to [DbAccount],
/// whereas contracts are identified by their code hash, and are stored in the `contracts` map.
/// The [DbAccount] holds the code hash of the contract, which is used to look up the contract in the `contracts` map.
#[derive(Debug, Clone)]
pub struct StateDB<ExtDB> {
    /// Account info where None means it is not existing. Not existing state is needed for Pre TANGERINE forks.
    /// `code` is always `None`, and bytecode can be found in `contracts`.
    pub accounts: HashMap<Address, DbAccount>,
    /// Tracks all contracts by their code hash.
    pub contracts: HashMap<B256, Bytecode>,
    /// All cached block hashes from the [DatabaseRef].
    pub block_hashes: HashMap<U256, B256>,
    /// The underlying database ([DatabaseRef]) that is used to load data.
    ///
    /// Note: this is read-only, data is never written to this database.
    pub db: ExtDB,

    pub original_accounts: HashMap<Address, DbAccount>,
    pub original_contracts: HashMap<B256, Bytecode>,
    pub original_block_hashes: HashMap<U256, B256>,
}

impl<ExtDB: Default> Default for StateDB<ExtDB> {
    fn default() -> Self {
        Self::new(ExtDB::default())
    }
}

impl<ExtDB> StateDB<ExtDB> {
    pub fn new(db: ExtDB) -> Self {
        let mut contracts = HashMap::default();
        contracts.insert(KECCAK_EMPTY, Bytecode::default());
        contracts.insert(B256::ZERO, Bytecode::default());

        Self {
            accounts: HashMap::default(),
            contracts,
            block_hashes: HashMap::default(),
            original_accounts: HashMap::default(),
            original_contracts: HashMap::default(),
            original_block_hashes: HashMap::default(),
            db,
        }
    }

    /// Inserts the account's code into the cache.
    ///
    /// Accounts objects and code are stored separately in the cache, this will take the code from the account and instead map it to the code hash.
    ///
    /// Note: This will not insert into the underlying external database.
    pub fn insert_contract(&mut self, account: &mut AccountInfo) {
        if let Some(code) = &account.code {
            if !code.is_empty() {
                if account.code_hash == KECCAK_EMPTY {
                    account.code_hash = code.hash_slow();
                }
                self.contracts.entry(account.code_hash).or_insert_with(|| code.clone());
            }
        }
        if account.code_hash.is_zero() {
            account.code_hash = KECCAK_EMPTY;
        }
    }

    /// Insert account info but not override storage
    pub fn insert_account_info(&mut self, address: Address, mut info: AccountInfo) {
        self.insert_contract(&mut info);
        self.accounts.entry(address).or_default().info = info;
    }
}

impl<ExtDB: DatabaseRef> StateDB<ExtDB> {
    pub fn new_with_state(db: ExtDB, pools: StdHashMap<String, Pool>) -> Self {
        let mut state_db = Self::new(db);
        tracing::debug!("Creating CacheDB with {} pools", pools.len());
        for (_address, pool) in pools {
            if pool.protocol == PoolProtocol::UniSwapV2 {
                let _ = state_db.handle_storage_v2(&pool);
                continue;
            }

            if pool.protocol == PoolProtocol::UniSwapV3 {
                continue;
                // TODO: Implement this
            }
        }
        tracing::debug!("CacheDB initialized!");
        state_db
    }

    pub fn clone(&self, db: ExtDB) -> Self {
        Self {
            db,
            contracts: self.original_contracts.clone(),
            accounts: self.original_accounts.clone(),
            block_hashes: self.original_block_hashes.clone(),
            original_block_hashes: self.original_block_hashes.clone(),
            original_contracts: self.original_contracts.clone(),
            original_accounts: self.original_accounts.clone(),
        }
    }

    pub fn sync_originals(&mut self) -> &mut Self {
        self.original_accounts = self.accounts.clone();
        self.original_contracts = self.contracts.clone();
        self.original_block_hashes = self.block_hashes.clone();
        return self;
    }

    fn handle_storage_v2(&mut self, pool: &Pool) -> Result<(), ExtDB::Error> {
        if pool.address.is_empty() {
            return Ok(());
        }
        let pair_addr = Address::from_str(pool.address.as_str()).unwrap();
        let token0_addr = Address::from_str(pool.token0.address.as_str()).unwrap();
        let token1_addr = Address::from_str(pool.token1.address.as_str()).unwrap();

        let v2_data = pool.v2_data.clone().unwrap();
        let mut storage: HashMap<U256, U256> = HashMap::default();
        storage.insert(U256::from(6), U256::from_str(pool.token0.address.as_str()).unwrap());
        storage.insert(U256::from(7), U256::from_str(pool.token1.address.as_str()).unwrap());
        storage.insert(U256::from(8), v2_data.clone().abi_encode());
        storage.insert(U256::from(9), U256::from(1));
        storage.insert(U256::from(10), U256::from(1));
        storage.insert(U256::from(12), U256::from(1));
        self.replace_account_storage(pair_addr, storage)?;

        self.insert_account_storage(
            token0_addr,
            pool.token0.hash_balance_slot(pair_addr).into(),
            v2_data.reserve_0.clone(),
        )?;

        self.insert_account_storage(
            token1_addr,
            pool.token1.hash_balance_slot(pair_addr).into(),
            v2_data.reserve_1.clone(),
        )?;

        Ok(())
    }

    /// Returns the account for the given address.
    ///
    /// If the account was not found in the cache, it will be loaded from the underlying database.
    pub fn load_account(&mut self, address: Address) -> Result<&mut DbAccount, ExtDB::Error> {
        let db = &self.db;
        match self.accounts.entry(address) {
            Entry::Occupied(entry) => Ok(entry.into_mut()),
            Entry::Vacant(entry) => {
                let account = db
                    .basic_ref(address)?
                    .map(|info| DbAccount { info, ..Default::default() })
                    .unwrap_or_else(DbAccount::new_not_existing);
                self.original_accounts.insert(address, account.clone());
                Ok(entry.insert(account))
            },
        }
    }

    /// insert account storage without overriding account info
    pub fn insert_account_storage(&mut self, address: Address, slot: U256, value: U256) -> Result<(), ExtDB::Error> {
        let account = self.load_account(address)?;
        account.storage.insert(slot, value);
        Ok(())
    }

    /// replace account storage without overriding account info
    pub fn replace_account_storage(
        &mut self,
        address: Address,
        storage: HashMap<U256, U256>,
    ) -> Result<(), ExtDB::Error> {
        let account = self.load_account(address)?;
        account.account_state = AccountState::StorageCleared;
        account.storage = storage.into_iter().collect();
        Ok(())
    }
}

impl<ExtDB> DatabaseCommit for StateDB<ExtDB> {
    fn commit(&mut self, changes: HashMap<Address, Account>) {
        let changes_size = changes.len();
        for (address, mut account) in changes {
            if !account.is_touched() {
                continue;
            }
            if account.is_selfdestructed() {
                let db_account = self.accounts.entry(address).or_default();
                db_account.storage.clear();
                db_account.account_state = AccountState::NotExisting;
                db_account.info = AccountInfo::default();
                continue;
            }
            let is_newly_created = account.is_created();
            self.insert_contract(&mut account.info);

            let db_account = self.accounts.entry(address).or_default();
            db_account.info = account.info;

            db_account.account_state = if is_newly_created {
                db_account.storage.clear();
                AccountState::StorageCleared
            } else if db_account.account_state.is_storage_cleared() {
                // Preserve old account state if it already exists
                AccountState::StorageCleared
            } else {
                AccountState::Touched
            };
            db_account
                .storage
                .extend(account.storage.into_iter().map(|(key, value)| (key, value.present_value())));
        }
        tracing::debug!("Committed to database, changed size: {}", changes_size);
    }
}

impl<ExtDB: DatabaseRef> Database for StateDB<ExtDB> {
    type Error = ExtDB::Error;

    fn basic(&mut self, address: Address) -> Result<Option<AccountInfo>, Self::Error> {
        let basic = match self.accounts.entry(address) {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(entry) => {
                let db_acc = self
                    .db
                    .basic_ref(address)?
                    .map(|info| DbAccount { info, ..Default::default() })
                    .unwrap_or_else(DbAccount::new_not_existing);
                self.original_accounts.insert(address, db_acc.clone());
                entry.insert(db_acc)
            },
        };
        Ok(basic.info())
    }

    fn code_by_hash(&mut self, code_hash: B256) -> Result<Bytecode, Self::Error> {
        match self.contracts.entry(code_hash) {
            Entry::Occupied(entry) => Ok(entry.get().clone()),
            Entry::Vacant(entry) => {
                // if you return code bytes when basic fn is called this function is not needed.
                let code = self.db.code_by_hash_ref(code_hash)?;
                self.original_contracts.insert(code_hash, code.clone());
                Ok(entry.insert(code).clone())
            },
        }
    }

    /// Get the value in an account's storage slot.
    ///
    /// It is assumed that account is already loaded.
    fn storage(&mut self, address: Address, index: U256) -> Result<U256, Self::Error> {
        match self.accounts.entry(address) {
            Entry::Occupied(mut acc_entry) => {
                let acc_entry = acc_entry.get_mut();
                match acc_entry.storage.entry(index) {
                    Entry::Occupied(entry) => Ok(*entry.get()),
                    Entry::Vacant(entry) => {
                        if matches!(
                            acc_entry.account_state,
                            AccountState::StorageCleared | AccountState::NotExisting
                        ) {
                            Ok(U256::ZERO)
                        } else {
                            let slot = self.db.storage_ref(address, index)?;
                            self.original_accounts.get_mut(&address).unwrap().storage.insert(index, slot);
                            entry.insert(slot);
                            Ok(slot)
                        }
                    },
                }
            },
            Entry::Vacant(acc_entry) => {
                // acc needs to be loaded for us to access slots.
                let info = self.db.basic_ref(address)?;
                let (account, value) = if info.is_some() {
                    let value = self.db.storage_ref(address, index)?;
                    let mut account: DbAccount = info.into();
                    account.storage.insert(index, value);
                    (account, value)
                } else {
                    (info.into(), U256::ZERO)
                };
                self.original_accounts.insert(address, account.clone());
                acc_entry.insert(account);
                Ok(value)
            },
        }
    }

    fn block_hash(&mut self, number: u64) -> Result<B256, Self::Error> {
        match self.block_hashes.entry(U256::from(number)) {
            Entry::Occupied(entry) => Ok(*entry.get()),
            Entry::Vacant(entry) => {
                let hash = self.db.block_hash_ref(number)?;
                self.original_block_hashes.insert(U256::from(number), hash);
                entry.insert(hash);
                Ok(hash)
            },
        }
    }
}

impl<ExtDB: DatabaseRef> DatabaseRef for StateDB<ExtDB> {
    type Error = ExtDB::Error;

    fn basic_ref(&self, address: Address) -> Result<Option<AccountInfo>, Self::Error> {
        match self.accounts.get(&address) {
            Some(acc) => Ok(acc.info()),
            None => self.db.basic_ref(address),
        }
    }

    fn code_by_hash_ref(&self, code_hash: B256) -> Result<Bytecode, Self::Error> {
        match self.contracts.get(&code_hash) {
            Some(entry) => Ok(entry.clone()),
            None => self.db.code_by_hash_ref(code_hash),
        }
    }

    fn storage_ref(&self, address: Address, index: U256) -> Result<U256, Self::Error> {
        match self.accounts.get(&address) {
            Some(acc_entry) => match acc_entry.storage.get(&index) {
                Some(entry) => Ok(*entry),
                None => {
                    if matches!(
                        acc_entry.account_state,
                        AccountState::StorageCleared | AccountState::NotExisting
                    ) {
                        Ok(U256::ZERO)
                    } else {
                        self.db.storage_ref(address, index)
                    }
                },
            },
            None => self.db.storage_ref(address, index),
        }
    }

    fn block_hash_ref(&self, number: u64) -> Result<B256, Self::Error> {
        match self.block_hashes.get(&U256::from(number)) {
            Some(entry) => Ok(*entry),
            None => self.db.block_hash_ref(number),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct DbAccount {
    pub info: AccountInfo,
    /// If account is selfdestructed or newly created, storage will be cleared.
    pub account_state: AccountState,
    /// storage slots
    pub storage: HashMap<U256, U256>,
}

impl DbAccount {
    pub fn new_not_existing() -> Self {
        Self {
            account_state: AccountState::NotExisting,
            ..Default::default()
        }
    }

    pub fn info(&self) -> Option<AccountInfo> {
        if matches!(self.account_state, AccountState::NotExisting) {
            None
        } else {
            Some(self.info.clone())
        }
    }
}

impl From<Option<AccountInfo>> for DbAccount {
    fn from(from: Option<AccountInfo>) -> Self {
        from.map(Self::from).unwrap_or_else(Self::new_not_existing)
    }
}

impl From<AccountInfo> for DbAccount {
    fn from(info: AccountInfo) -> Self {
        Self {
            info,
            account_state: AccountState::None,
            ..Default::default()
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub enum AccountState {
    /// Before Spurious Dragon hardfork there was a difference between empty and not existing.
    /// And we are flagging it here.
    NotExisting,
    /// EVM touched this account. For newer hardfork this means it can be cleared/removed from state.
    Touched,
    /// EVM cleared storage of this account, mostly by selfdestruct, we don't ask database for storage slots
    /// and assume they are U256::ZERO
    StorageCleared,
    /// EVM didn't interacted with this account
    #[default]
    None,
}

impl AccountState {
    /// Returns `true` if EVM cleared storage of this account
    pub fn is_storage_cleared(&self) -> bool {
        matches!(self, AccountState::StorageCleared)
    }
}

#[cfg(test)]
mod tests {
    use crate::simulation::state_db::StateDB;
    use revm::{
        db::EmptyDB,
        primitives::{db::Database, AccountInfo, Address, HashMap, U256},
    };

    #[test]
    fn test_insert_account_storage() {
        let account = Address::with_last_byte(42);
        let nonce = 42;
        let mut init_state = StateDB::new(EmptyDB::default());
        init_state.insert_account_info(account, AccountInfo { nonce, ..Default::default() });

        let (key, value) = (U256::from(123), U256::from(456));
        let mut new_state = StateDB::new(init_state);
        new_state.insert_account_storage(account, key, value).unwrap();

        assert_eq!(new_state.basic(account).unwrap().unwrap().nonce, nonce);
        assert_eq!(new_state.storage(account, key), Ok(value));
    }

    #[test]
    fn test_replace_account_storage() {
        let account = Address::with_last_byte(42);
        let nonce = 42;
        let mut init_state = StateDB::new(EmptyDB::default());
        init_state.insert_account_info(account, AccountInfo { nonce, ..Default::default() });

        let (key0, value0) = (U256::from(123), U256::from(456));
        let (key1, value1) = (U256::from(789), U256::from(999));
        init_state.insert_account_storage(account, key0, value0).unwrap();

        let mut new_state = StateDB::new(init_state);
        new_state
            .replace_account_storage(account, HashMap::from_iter([(key1, value1)]))
            .unwrap();

        assert_eq!(new_state.basic(account).unwrap().unwrap().nonce, nonce);
        assert_eq!(new_state.storage(account, key0), Ok(U256::ZERO));
        assert_eq!(new_state.storage(account, key1), Ok(value1));
    }
}
