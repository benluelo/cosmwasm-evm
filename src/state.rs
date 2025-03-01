use bytemuck::{Pod, Zeroable};
use cosmwasm_std::Storage;
use revm::primitives::{AccountInfo, Address, B256, Bytecode, U256, hex};

const ACCOUNT_INFO_PREFIX: u8 = 0x0;
const ACCOUNT_STORAGE_PREFIX: u8 = 0x1;
const CONTRACTS_PREFIX: u8 = 0x2;
const CONFIG_PREFIX: u8 = 0x3;

pub trait Store {
    type K;
    type V;

    fn encode_key(key: Self::K) -> impl AsRef<[u8]>;

    fn encode(value: &Self::V) -> Vec<u8>;

    fn decode(bz: &[u8]) -> Self::V;
}

#[derive(Clone, Copy, Pod, Zeroable)]
#[repr(C, packed)]
struct RawKey<const N: usize, const M: usize>([u8; N], [u8; M])
where
    [u8; N]: Pod + Zeroable,
    [u8; M]: Pod + Zeroable;

pub struct Config {
    pub denom: String,
}

pub enum ConfigStore {}

impl Store for ConfigStore {
    type K = ();

    type V = Config;

    fn encode_key((): Self::K) -> impl AsRef<[u8]> {
        [CONFIG_PREFIX]
    }

    fn encode(value: &Self::V) -> Vec<u8> {
        value.denom.as_bytes().into()
    }

    fn decode(bz: &[u8]) -> Self::V {
        Config {
            denom: String::from_utf8(bz.to_vec()).expect("bad storage"),
        }
    }
}

#[derive(Debug, Clone, Copy, Pod, Zeroable)]
#[repr(C)]
pub struct RawAccountInfo {
    pub balance: [u8; 32],
    pub nonce: u64,
    pub code_hash: [u8; 32],
}

impl Default for RawAccountInfo {
    fn default() -> Self {
        Self {
            balance: Default::default(),
            nonce: Default::default(),
            code_hash: hex!("c5d2460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470"),
        }
    }
}

impl RawAccountInfo {
    #[must_use]
    pub fn new(balance: U256, nonce: u64, code_hash: B256) -> Self {
        Self {
            balance: balance.to_be_bytes(),
            nonce,
            code_hash: code_hash.into(),
        }
    }
}

impl From<RawAccountInfo> for AccountInfo {
    fn from(value: RawAccountInfo) -> Self {
        Self {
            balance: U256::from_be_bytes::<32>(value.balance),
            nonce: value.nonce,
            code_hash: value.code_hash.into(),
            code: None,
        }
    }
}

pub enum AccountInfoStore {}

impl Store for AccountInfoStore {
    type K = Address;

    type V = RawAccountInfo;

    fn encode_key(address: Self::K) -> impl AsRef<[u8]> {
        bytemuck::must_cast::<_, [u8; 21]>(RawKey([ACCOUNT_INFO_PREFIX], address.0.0))
    }

    fn encode(value: &Self::V) -> Vec<u8> {
        bytemuck::must_cast_ref::<Self::V, [u8; 72]>(value).to_vec()
    }

    fn decode(bz: &[u8]) -> Self::V {
        bytemuck::must_cast::<[u8; 72], Self::V>(bz.try_into().expect("bad storage"))
    }
}

pub enum AccountStorageStore {}

impl Store for AccountStorageStore {
    type K = (Address, U256);

    type V = U256;

    fn encode_key((address, slot): Self::K) -> impl AsRef<[u8]> {
        bytemuck::must_cast::<_, [u8; 53]>(RawKey(
            [ACCOUNT_STORAGE_PREFIX],
            bytemuck::must_cast::<_, [u8; 52]>(RawKey(address.0.0, slot.to_be_bytes::<32>())),
        ))
    }

    fn encode(value: &Self::V) -> Vec<u8> {
        value.to_be_bytes::<32>().to_vec()
    }

    fn decode(bz: &[u8]) -> Self::V {
        U256::from_be_bytes::<32>(bz.try_into().expect("bad storage"))
    }
}

pub enum ContractsStore {}

impl Store for ContractsStore {
    type K = B256;

    type V = Bytecode;

    fn encode_key(hash: Self::K) -> impl AsRef<[u8]> {
        bytemuck::must_cast::<_, [u8; 33]>(RawKey([CONTRACTS_PREFIX], hash.0))
    }

    fn encode(value: &Self::V) -> Vec<u8> {
        value.bytes().into()
    }

    fn decode(bz: &[u8]) -> Self::V {
        Bytecode::new_raw(bz.to_vec().into())
    }
}

pub trait StorageExt {
    fn read<T: Store>(&self, k: T::K) -> Option<T::V>;

    fn write<T: Store>(&mut self, k: T::K, v: &T::V);
}

impl StorageExt for dyn Storage + '_ {
    fn read<T: Store>(&self, key: T::K) -> Option<T::V> {
        self.get(T::encode_key(key).as_ref())
            .map(|raw| T::decode(&raw))
    }

    fn write<T: Store>(&mut self, k: T::K, v: &T::V) {
        self.set(T::encode_key(k).as_ref(), T::encode(v).as_ref());
    }
}
