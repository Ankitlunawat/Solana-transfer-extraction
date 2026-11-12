mod decoder;
mod output;

use std::collections::HashMap;

use decoder::{Instruction, TransferKind};
pub use output::{Transfer, Transfers};
use substreams::errors::Error;
use substreams_solana::{base58, pb::sf::solana::r#type::v1::Block};

#[substreams::handlers::map]
pub fn map_transfers(block: Block) -> Result<Transfers, Error> {
    let mut transfers = Vec::new();
    for tx in block.transactions() {
        let Some(meta) = &tx.meta else { continue };
        // Instructions in a failed Solana transaction do not represent committed transfers.
        if meta.err.is_some() {
            continue;
        }
        let Some(message) = tx.transaction.as_ref().and_then(|tx| tx.message.as_ref()) else {
            continue;
        };

        let keys: Vec<&[u8]> = message
            .account_keys
            .iter()
            .chain(&meta.loaded_writable_addresses)
            .chain(&meta.loaded_readonly_addresses)
            .map(Vec::as_slice)
            .collect();

        // Index CPI groups once so large transactions do not repeatedly scan them.
        let inner_by_parent: HashMap<_, _> = meta
            .inner_instructions
            .iter()
            .map(|group| (group.index as usize, &group.instructions))
            .collect();
        let keys = keys.as_slice();
        let instructions = || {
            message
                .instructions
                .iter()
                .enumerate()
                .flat_map(|(index, ix)| {
                    let outer = Instruction::new(keys, ix.program_id_index, &ix.accounts, &ix.data);
                    let inner = inner_by_parent
                        .get(&index)
                        .into_iter()
                        .flat_map(|group| group.iter())
                        .map(move |ix| {
                            Instruction::new(keys, ix.program_id_index, &ix.accounts, &ix.data)
                        });
                    std::iter::once(outer).chain(inner).flatten()
                })
        };

        // Pre/post balances cover most accounts. Initialization also covers temporary accounts.
        let mut owners: HashMap<_, _> = instructions()
            .filter_map(|ix| ix.initialized_owner())
            .collect();
        for balance in meta
            .pre_token_balances
            .iter()
            .chain(&meta.post_token_balances)
        {
            if let Ok(index) = u8::try_from(balance.account_index) {
                if !balance.owner.is_empty() {
                    owners.insert(index, balance.owner.clone());
                }
            }
        }

        for ix in instructions() {
            let Some(decoded) = ix.transfer() else {
                continue;
            };
            let address = |index: u8| {
                let key = keys.get(usize::from(index))?;
                if decoded.kind == TransferKind::Token {
                    if let Some(owner) = owners.get(&index) {
                        return Some(owner.clone());
                    }
                }
                Some(base58::encode(key))
            };
            if let (Some(from), Some(to)) = (address(decoded.source), address(decoded.destination))
            {
                transfers.push(Transfer {
                    from,
                    to,
                    amount: decoded.amount,
                });
            }
        }
    }
    Ok(Transfers {
        transfers,
        slot: block.slot,
    })
}
