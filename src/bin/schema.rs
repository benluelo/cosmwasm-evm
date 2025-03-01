use cosmwasm_evm::msg::{ExecuteMsg, InstantiateMsg, MigrateMsg};
use cosmwasm_schema::write_api;

fn main() {
    write_api! {
        instantiate: InstantiateMsg,
        execute: ExecuteMsg,
        // query: QueryMsg,
        migrate: MigrateMsg,
    }
}
