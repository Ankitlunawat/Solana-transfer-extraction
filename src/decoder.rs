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
    pub fn new(
        keys: &'a [&'a [u8]],
        program: u32,
        accounts: &'a [u8],
        data: &'a [u8],
    ) -> Option<Self> {
        Some(Self {
            keys,
            program: keys.get(program as usize)?,
            accounts,
            data,
        })
    }

    fn is_token(&self) -> bool {
        self.program == SPL_TOKEN || self.program == TOKEN_2022
    }

    pub fn transfer(&self) -> Option<DecodedTransfer> {
        let (kind, destination, offset) = if self.program == SYSTEM {
            let tag = u32::from_le_bytes(self.data.get(..4)?.try_into().ok()?);
            match tag {
                2 if self.data.len() == 12 => (TransferKind::Native, 1, 4),
                11 => {
                    // Bincode SystemInstruction::TransferWithSeed:
                    // tag(u32), lamports(u64), seed_length(u64), seed, owner([u8; 32]).
                    // Reject truncated or malformed instructions rather than treating
                    // a partial payload as a successful native SOL transfer.
                    let seed_len = u64::from_le_bytes(self.data.get(12..20)?.try_into().ok()?);
                    if seed_len > 32 {
                        return None;
                    }
                    let total_len = 20usize.checked_add(seed_len as usize)?.checked_add(32)?;
                    if self.data.len() != total_len || self.accounts.len() < 3 {
                        return None;
                    }
                    (TransferKind::Native, 2, 4)
                }
                _ => return None,
            }
        } else if self.is_token() {
            match self.data.first()? {
                // Transfer: discriminator + amount.
                3 if self.data.len() == 9 => (TransferKind::Token, 1, 1),
                // TransferChecked: discriminator + amount + decimals.
                12 if self.data.len() == 10 => (TransferKind::Token, 2, 1),
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
        let mut data = Vec::new();
        data.extend_from_slice(&11_u32.to_le_bytes());
        data.extend_from_slice(&325253_u64.to_le_bytes());
        data.extend_from_slice(&4_u64.to_le_bytes()); // bincode seed length
        data.extend_from_slice(b"seed");
        data.extend_from_slice(&[42_u8; 32]); // from_owner public key
        let ix = Instruction::new(&keys, 0, &[7, 8, 9], &data).unwrap();
        assert_eq!(
            ix.transfer().unwrap(),
            DecodedTransfer {
                kind: TransferKind::Native,
                source: 7,
                destination: 9,
                amount: 325253,
            }
        );
    }

    #[test]
    fn seeded_transfer_rejects_truncated_or_malformed_payloads() {
        let keys = [SYSTEM.as_slice()];
        let mut data = Vec::new();
        data.extend_from_slice(&11_u32.to_le_bytes());
        data.extend_from_slice(&325253_u64.to_le_bytes());
        data.extend_from_slice(&4_u64.to_le_bytes());
        data.extend_from_slice(b"seed");
        data.extend_from_slice(&[42_u8; 32]);

        for length in [12, 19, 20, 23, 24, 55] {
            let ix = Instruction::new(&keys, 0, &[7, 8, 9], &data[..length]).unwrap();
            assert!(
                ix.transfer().is_none(),
                "accepted truncated length {length}"
            );
        }

        let mut invalid_seed_length = data.clone();
        invalid_seed_length[12..20].copy_from_slice(&33_u64.to_le_bytes());
        assert!(Instruction::new(&keys, 0, &[7, 8, 9], &invalid_seed_length)
            .unwrap()
            .transfer()
            .is_none());

        assert!(Instruction::new(&keys, 0, &[7, 8], &data)
            .unwrap()
            .transfer()
            .is_none());
        assert!(
            Instruction::new(&keys, 0, &[7, 8, 9], &[data.as_slice(), &[0]].concat())
                .unwrap()
                .transfer()
                .is_none()
        );
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
        assert!(Instruction::new(&keys, 0, &[4, 5, 6], &data[..9])
            .unwrap()
            .transfer()
            .is_none());
        let mut trailing = data.to_vec();
        trailing.push(0);
        assert!(Instruction::new(&keys, 0, &[4, 5, 6], &trailing)
            .unwrap()
            .transfer()
            .is_none());

        let mut unchecked = [0; 9];
        unchecked[0] = 3;
        unchecked[1..9].copy_from_slice(&u64::MAX.to_le_bytes());
        assert_eq!(
            Instruction::new(&keys, 0, &[4, 5], &unchecked)
                .unwrap()
                .transfer()
                .unwrap()
                .amount,
            u64::MAX
        );
        let mut unchecked_trailing = unchecked.to_vec();
        unchecked_trailing.push(0);
        assert!(Instruction::new(&keys, 0, &[4, 5], &unchecked_trailing)
            .unwrap()
            .transfer()
            .is_none());
    }

    #[test]
    fn rejects_wrong_program_and_invalid_account_indexes() {
        let wrong_program = [1; 32];
        let keys = [wrong_program.as_slice()];
        assert!(
            Instruction::new(&keys, 0, &[0, 1], &[2, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0])
                .unwrap()
                .transfer()
                .is_none()
        );

        let keys = [SYSTEM.as_slice()];
        assert!(
            Instruction::new(&keys, 0, &[0], &[2, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0])
                .unwrap()
                .transfer()
                .is_none()
        );
        assert!(
            Instruction::new(&keys, 0, &[0, 1, 2], &[2, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0])
                .unwrap()
                .transfer()
                .is_some()
        );
    }

    #[test]
    fn resolves_temporary_token_account_owner() {
        let keys = [SPL_TOKEN.as_slice()];
        let mut data = [7; 33];
        data[0] = 18;
        let ix = Instruction::new(&keys, 0, &[5], &data).unwrap();
        assert_eq!(
            ix.initialized_owner(),
            Some((5, base58::encode(&data[1..])))
        );
    }
}
