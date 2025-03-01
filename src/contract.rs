use cosmwasm_std::{
    Coin, CosmosMsg, DepsMut, Env, Event, MessageInfo, OverflowError, OverflowOperation, Response,
    StdError, StdResult, SubMsg, Uint128, Uint256, entry_point,
};
use revm::primitives::{Address, ExecutionResult, Output, SuccessReason, U256};
use sha2::Digest;

use crate::{
    evm::Evm,
    msg::{ExecuteMsg, InstantiateMsg, MigrateMsg, Tx, TxKind},
    state::{AccountInfoStore, Config, ConfigStore, RawAccountInfo, StorageExt},
};

#[entry_point]
#[allow(clippy::needless_pass_by_value)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> StdResult<Response> {
    deps.storage.write::<ConfigStore>(
        (),
        &Config {
            denom: msg.eth_token,
        },
    );

    Ok(Response::default())
}

// #[entry_point]
// #[allow(clippy::needless_pass_by_value)]
// pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
//     match msg {
//         QueryMsg::LinkedAddress { github_user_id } => Ok(to_json_binary(
//             &USERS.may_load(deps.storage, github_user_id)?,
//         )?),
//     }
// }

#[entry_point]
#[allow(clippy::needless_pass_by_value)]
pub fn execute(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> StdResult<Response> {
    let sender = Address::from_private_key(
        &Into::<[u8; 32]>::into(
            sha2::Sha256::new()
                .chain_update(info.sender.as_bytes())
                .finalize(),
        )
        .as_slice()
        .try_into()
        .expect("32 bytes is a valid private key; qed;"),
    );

    match msg {
        ExecuteMsg::Transaction(tx) => {
            let mut evm = Evm::new(deps.storage);

            transaction(&mut evm, sender, tx)
        }
        ExecuteMsg::Lock => {
            let config = deps
                .storage
                .read::<ConfigStore>(())
                .expect("config must exist");

            let funds = info
                .funds
                .iter()
                .find(|c| c.denom == config.denom)
                .ok_or_else(|| StdError::generic_err("no funds provided"))?;

            let account = deps
                .storage
                .read::<AccountInfoStore>(sender)
                .unwrap_or_default();

            deps.storage.write::<AccountInfoStore>(
                sender,
                &RawAccountInfo {
                    balance: (U256::from_be_bytes::<32>(account.balance)
                        + U256::from(funds.amount.u128()))
                    .to_be_bytes(),
                    nonce: account.nonce,
                    code_hash: account.code_hash,
                },
            );

            Ok(Response::new().add_event(
                Event::new("lock")
                    .add_attribute("ether", funds.amount)
                    .add_attribute("address", sender.to_string()),
            ))
        }
        ExecuteMsg::Unlock(eth) => {
            let eth_ = U256::from_be_bytes(eth.to_be_bytes());

            let config = deps
                .storage
                .read::<ConfigStore>(())
                .expect("config must exist");

            let account = deps
                .storage
                .read::<AccountInfoStore>(sender)
                .unwrap_or_default();

            let current_balance = U256::from_be_bytes::<32>(account.balance);

            if current_balance < eth_ {
                return Err(StdError::overflow(OverflowError::new(
                    OverflowOperation::Sub,
                )));
            }

            deps.storage.write::<AccountInfoStore>(
                sender,
                &RawAccountInfo {
                    balance: (current_balance - eth_).to_be_bytes(),
                    nonce: account.nonce,
                    code_hash: account.code_hash,
                },
            );

            Ok(Response::new()
                .add_event(
                    Event::new("unlock")
                        .add_attribute("ether", Uint256::from_be_bytes(eth_.to_be_bytes()))
                        .add_attribute("address", sender.to_string()),
                )
                .add_submessage(SubMsg::reply_never(CosmosMsg::Bank(
                    cosmwasm_std::BankMsg::Send {
                        to_address: info.sender.to_string(),
                        amount: vec![Coin::new(Uint128::try_from(eth)?, config.denom)],
                    },
                ))))
        }
    }
}

pub fn transaction(evm: &mut Evm, sender: Address, tx: Tx) -> StdResult<Response> {
    let tx_mut = evm.evm.tx_mut();

    tx_mut.caller = sender;
    tx_mut.gas_limit = u64::MAX;
    tx_mut.gas_price = U256::from(0);
    tx_mut.transact_to = match tx.to {
        TxKind::Create => revm::primitives::TxKind::Create,
        TxKind::Call(addr) => revm::primitives::TxKind::Call(addr.0),
    };
    tx_mut.value = tx
        .value
        .map(|value| U256::from_be_bytes(value.to_be_bytes()))
        .unwrap_or_default();
    tx_mut.data = tx.input.unwrap_or_default().to_vec().into();

    let res = evm.evm.transact_commit();

    match res {
        Ok(ExecutionResult::Success {
            reason,
            gas_used,
            gas_refunded,
            logs,
            output,
        }) => Ok(Response::new()
            .add_event(
                Event::new("evm").add_attributes([
                    ("caller", sender.to_string()),
                    (
                        "reason",
                        match reason {
                            SuccessReason::Stop => "stop",
                            SuccessReason::Return => "return",
                            SuccessReason::SelfDestruct => "self_destruct",
                            SuccessReason::EofReturnContract => "eof_return_contract",
                        }
                        .to_owned(),
                    ),
                    ("gas_used", gas_used.to_string()),
                    ("gas_refunded", gas_refunded.to_string()),
                ]),
            )
            .add_event(match output {
                Output::Call(bytes) => Event::new("call").add_attribute("value", bytes.to_string()),
                Output::Create(bytes, address) => Event::new("create")
                    .add_attribute("value", bytes.to_string())
                    .add_attribute(
                        "address",
                        address.map(|a| a.to_string()).unwrap_or_default(),
                    ),
            })
            .add_events(logs.into_iter().map(|log| {
                Event::new("log")
                    .add_attribute("address", log.address.to_string())
                    .add_attributes(
                        log.topics()
                            .iter()
                            .enumerate()
                            .map(|(idx, topic)| (format!("topic{idx}"), topic.to_string())),
                    )
                    .add_attribute("data", log.data.data.to_string())
            }))),
        Ok(res) => Err(StdError::generic_err(
            serde_json::to_string(&res).expect("infallible"),
        )),
        Err(err) => Err(StdError::generic_err(err.to_string())),
    }
}

#[entry_point]
pub fn migrate(_: DepsMut, _: Env, _: MigrateMsg) -> StdResult<Response> {
    Ok(Response::default())
}

#[cfg(test)]
mod tests {
    use cosmwasm_std::{
        Coin,
        testing::{message_info, mock_dependencies, mock_env},
    };
    use revm::primitives::{address, hex};

    use crate::{
        contract::execute,
        msg::{Addr, Tx, TxKind},
    };

    use super::*;

    #[test]
    fn exec() {
        let mut deps = mock_dependencies();
        let env = mock_env();
        let info = message_info(&deps.api.addr_make(""), &[Coin::new(100_u128, "denom")]);

        let info_with_funds =
            message_info(&deps.api.addr_make(""), &[Coin::new(100_u128, "denom")]).clone();

        instantiate(
            deps.as_mut(),
            env.clone(),
            info.clone(),
            InstantiateMsg {
                eth_token: "denom".to_owned(),
            },
        )
        .unwrap();

        let res = execute(
            deps.as_mut(),
            env.clone(),
            info_with_funds,
            ExecuteMsg::Lock,
        )
        .unwrap();

        dbg!(res);

        let res = execute(
            deps.as_mut(),
            env.clone(),
            info.clone(),
            ExecuteMsg::Transaction(Tx {
                // from: Some(Addr(Address::default())),
                to: TxKind::Create,
                value: None,
                input: Some(hex!("6080604052348015600e575f80fd5b506040516104163803806104168339818101604052810190602e9190606b565b805f81905550506091565b5f80fd5b5f819050919050565b604d81603d565b81146056575f80fd5b50565b5f815190506065816046565b92915050565b5f60208284031215607d57607c6039565b5b5f6088848285016059565b91505092915050565b6103788061009e5f395ff3fe608060405234801561000f575f80fd5b506004361061004a575f3560e01c806306661abd1461004e5780636d4ce63c1461006c578063b3bcfa821461008a578063fc5842bd14610094575b5f80fd5b6100566100b0565b60405161006391906101e2565b60405180910390f35b6100746100b5565b60405161008191906101e2565b60405180910390f35b6100926100bd565b005b6100ae60048036038101906100a99190610235565b61010f565b005b5f5481565b5f8054905090565b60015f808282546100ce919061028d565b925050819055507f757fff3e831f63e329ee929d928e44a48df56c5abd902d2414c60211a993e37e5f5460405161010591906101e2565b60405180910390a1565b600a8160ff16111561015857806040517fe74246a900000000000000000000000000000000000000000000000000000000815260040161014f91906102cf565b60405180910390fd5b5b5f8160ff1611156101c75760015f8082825461017591906102e8565b925050819055507f3443590b7333fb7cfd5e65585c8a4c4100c345929865db522919623bf37e58085f546040516101ac91906101e2565b60405180910390a180806101bf9061031b565b915050610159565b50565b5f819050919050565b6101dc816101ca565b82525050565b5f6020820190506101f55f8301846101d3565b92915050565b5f80fd5b5f60ff82169050919050565b610214816101ff565b811461021e575f80fd5b50565b5f8135905061022f8161020b565b92915050565b5f6020828403121561024a576102496101fb565b5b5f61025784828501610221565b91505092915050565b7f4e487b71000000000000000000000000000000000000000000000000000000005f52601160045260245ffd5b5f610297826101ca565b91506102a2836101ca565b92508282039050818111156102ba576102b9610260565b5b92915050565b6102c9816101ff565b82525050565b5f6020820190506102e25f8301846102c0565b92915050565b5f6102f2826101ca565b91506102fd836101ca565b925082820190508082111561031557610314610260565b5b92915050565b5f610325826101ff565b91505f820361033757610336610260565b5b60018203905091905056fea2646970667358221220756fc4b018cad6c146571e79e451bd4b6acc78da96ede40416af47a594d271f064736f6c634300081a00330000000000000000000000000000000000000000000000000000000000000001").into()),
                nonce: None,
                chain_id: None,
                transaction_type: None,
            }),
        );

        dbg!(res).unwrap();

        let res = execute(
            deps.as_mut(),
            env,
            info,
            ExecuteMsg::Transaction(Tx {
                // from: Some(Addr(Address::default())),
                to: TxKind::Call(Addr(address!("0x1c080665c72c0b9306d4319c0cce4ed153579863"))),
                value: None,
                input: Some(
                    hex!(
                        "fc5842bd0000000000000000000000000000000000000000000000000000000000000008"
                    )
                    .into(),
                ),
                nonce: None,
                chain_id: None,
                transaction_type: None,
            }),
        );

        dbg!(&deps.storage);

        dbg!(res).unwrap();

        // let msg = serde_json::from_str(r#"{"transaction":{"input":"","to":"create"}}"#);
    }
}
