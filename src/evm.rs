use std::convert::Infallible;

use cosmwasm_std::Storage;
use revm::{
    Database, DatabaseCommit,
    primitives::{Account, AccountInfo, Address, B256, Bytecode, HashMap, U256},
};

use crate::state::{
    AccountInfoStore, AccountStorageStore, ContractsStore, RawAccountInfo, StorageExt,
};

pub struct Evm<'a> {
    pub evm: revm::Evm<'a, (), CwDb<'a>>,
}

impl<'a> Evm<'a> {
    #[must_use]
    pub fn new(storage: &'a mut dyn Storage) -> Self {
        Self {
            evm: revm::Evm::builder()
                .with_spec_id(revm::primitives::SpecId::LATEST)
                .with_db(CwDb { storage })
                .build(),
        }
    }
}

pub struct CwDb<'a> {
    storage: &'a mut dyn Storage,
}

pub const ADDRESS_PREFIX: u8 = 0x00;
pub const CODE_PREFIX: u8 = 0x01;
pub const BLOCK_HASH_PREFIX: u8 = 0x02;

impl Database for CwDb<'_> {
    type Error = Infallible;

    fn basic(&mut self, address: Address) -> Result<Option<AccountInfo>, Self::Error> {
        Ok(self
            .storage
            .read::<AccountInfoStore>(address)
            .map(Into::into))
    }

    fn code_by_hash(&mut self, code_hash: B256) -> Result<Bytecode, Self::Error> {
        Ok(self
            .storage
            .read::<ContractsStore>(code_hash)
            .unwrap_or_default())
    }

    fn storage(&mut self, address: Address, index: U256) -> Result<U256, Self::Error> {
        Ok(self
            .storage
            .read::<AccountStorageStore>((address, index))
            .unwrap_or_default())
    }

    fn block_hash(&mut self, _number: u64) -> Result<B256, Self::Error> {
        todo!()
    }
}

impl DatabaseCommit for CwDb<'_> {
    fn commit(&mut self, changes: HashMap<Address, Account>) {
        for (address, mut account) in changes {
            if let Some(code) = account.info.code.take() {
                self.storage
                    .write::<ContractsStore>(account.info.code_hash, &code);
            }

            for (slot, value) in account.changed_storage_slots() {
                self.storage
                    .write::<AccountStorageStore>((address, *slot), &value.present_value);
            }

            self.storage.write::<AccountInfoStore>(
                address,
                &RawAccountInfo::new(
                    account.info.balance,
                    account.info.nonce,
                    account.info.code_hash,
                ),
            );
        }
    }
}
