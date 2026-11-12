use substreams_solana::{b58, base58};

const SYSTEM: [u8; 32] = [0; 32];
const SPL_TOKEN: [u8; 32] = b58!("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");
const TOKEN_2022: [u8; 32] = b58!("TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb");

#[derive(Debug, PartialEq, Eq)]
pub enum TransferKind {
    Native,
    Token,
}

#[derive(Debug, PartialEq, Eq)]
pub struct DecodedTransfer {
    pub kind: TransferKind,
    pub source: u8,
    pub destination: u8,
    pub amount: u64,
}

pub struct Instruction<'a> {
    keys: &'a [&'a [u8]],
    program: &'a [u8],
    accounts: &'a [u8],
    data: &'a [u8],
}

impl<'a> Instruction<'a> {
    pub fn new(keys: &'a [&'a [u8]], program: u32, accounts: &'a [u8], data: &'a [u8]) -> Option<Self> {
        Some(Self { keys, program: keys.get(program as usize)?, accounts, data })
    }

    fn is_token(&self) -> bool {
        self.program == SPL_TOKEN || self.program == TOKEN_2022
    }

    pub fn transfer(&self) -> Option<DecodedTransfer> {
        let (kind, destination, offset) = if self.program == SYSTEM {
            let tag = u32::from_le_bytes(self.data.get(..4)?.try_into().ok()?);
            match tag {
                2 => (TransferKind::Native, 1, 4),
                11 => (TransferKind::Native, 2, 4),
                _ => return None,
            }
        } else if self.is_token() {
            match self.data.first()? {
                3 => (TransferKind::Token, 1, 1),
                12 if self.data.len() >= 10 => (TransferKind::Token, 2, 1),
                _ => return None,
            }
        } else {
            return None;
        };

        Some(DecodedTransfer {
            kind,
            source: *self.accounts.first()?,
            destination: *self.accounts.get(destination)?,
            amount: u64::from_le_bytes(self.data.get(offset..offset + 8)?.try_into().ok()?),
        })
    }

    pub fn initialized_owner(&self) -> Option<(u8, String)> {
        if !self.is_token() {
            return None;
        }
        let owner = match self.data.first()? {
            1 => *self.keys.get(usize::from(*self.accounts.get(2)?))?,
            16 | 18 => self.data.get(1..33)?,
            _ => return None,
        };
        Some((*self.accounts.first()?, base58::encode(owner)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seeded_transfer_uses_recipient_after_signing_base() {
        let keys = [SYSTEM.as_slice()];
        let mut data = [0; 12];
        data[..4].copy_from_slice(&11_u32.to_le_bytes());
        data[4..].copy_from_slice(&325253_u64.to_le_bytes());
        let ix = Instruction::new(&keys, 0, &[7, 8, 9], &data).unwrap();
        assert_eq!(ix.transfer().unwrap(), DecodedTransfer {
            kind: TransferKind::Native, source: 7, destination: 9, amount: 325253,
        });
    }

    #[test]
    fn checked_token_transfer_requires_decimals_and_keeps_full_amount() {
        let keys = [TOKEN_2022.as_slice()];
        let mut data = [0; 10];
        data[0] = 12;
        data[1..9].copy_from_slice(&u64::MAX.to_le_bytes());
        let ix = Instruction::new(&keys, 0, &[4, 5, 6], &data).unwrap();
        let transfer = ix.transfer().unwrap();
        assert_eq!((transfer.destination, transfer.amount), (6, u64::MAX));
        assert!(Instruction::new(&keys, 0, &[4, 5, 6], &data[..9]).unwrap().transfer().is_none());
    }

    #[test]
    fn resolves_temporary_token_account_owner() {
        let keys = [SPL_TOKEN.as_slice()];
        let mut data = [7; 33];
        data[0] = 18;
        let ix = Instruction::new(&keys, 0, &[5], &data).unwrap();
        assert_eq!(ix.initialized_owner(), Some((5, base58::encode(&data[1..]))));
    }
}
