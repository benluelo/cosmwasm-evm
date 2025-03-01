use cosmwasm_schema::{
    cw_serde,
    serde::{Deserialize, Serialize},
};
use cosmwasm_std::{HexBinary, Uint256};
use revm::primitives::Address;

#[cw_serde]
pub struct InstantiateMsg {
    pub eth_token: String,
}

#[cw_serde]
pub enum ExecuteMsg {
    Transaction(Tx),
    Lock,
    Unlock(Uint256),
}

#[cw_serde]
pub struct MigrateMsg {}

/// Represents _all_ transaction requests to/from RPC.
#[cw_serde]
pub struct Tx {
    /// The destination address of the transaction.
    pub to: TxKind,
    /// The value transferred in the transaction, in wei.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<Uint256>,
    /// Transaction data.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input: Option<HexBinary>,
    /// The nonce of the transaction.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nonce: Option<u64>,
    /// The chain ID for the transaction.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chain_id: Option<u64>,
    #[serde(default, rename = "type", skip_serializing_if = "Option::is_none")]
    #[doc(alias = "tx_type")]
    pub transaction_type: Option<u8>,
}

/// The `to` field of a transaction. Either a target address, or empty for a
/// contract creation.
#[cw_serde]
pub enum TxKind {
    /// A transaction that creates a contract.
    Create,
    /// A transaction that calls a contract or transfer.
    Call(Addr),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(crate = "cosmwasm_schema::serde")]
pub struct Addr(pub Address);

impl cosmwasm_schema::schemars::JsonSchema for Addr {
    fn schema_name() -> String {
        "Address".to_owned()
    }

    fn json_schema(
        _g: &mut cosmwasm_schema::schemars::r#gen::SchemaGenerator,
    ) -> cosmwasm_schema::schemars::schema::Schema {
        cosmwasm_schema::schemars::schema::Schema::Object(
            cosmwasm_schema::schemars::schema::SchemaObject {
                metadata: Some(Box::new(cosmwasm_schema::schemars::schema::Metadata {
                    description: Some("An ethereum address".to_owned()),
                    ..Default::default()
                })),
                instance_type: Some(cosmwasm_schema::schemars::schema::SingleOrVec::Single(
                    Box::new(cosmwasm_schema::schemars::schema::InstanceType::String),
                )),
                string: Some(Box::new(
                    cosmwasm_schema::schemars::schema::StringValidation {
                        max_length: Some(42),
                        min_length: Some(42),
                        pattern: Some("^0x[0-9a-fA-F]{40}$".to_owned()),
                    },
                )),
                ..Default::default()
            },
        )
    }
}
