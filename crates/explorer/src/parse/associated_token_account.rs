use crate::parse::{
    check_num_accounts, ParsableProgram, ParseInstructionError, ParsedInstructionEnum,
};
use serde_json::json;
use solana_sdk::{instruction::CompiledInstruction, pubkey::Pubkey};

pub fn parse_associated_token_account(
    instruction: &CompiledInstruction,
    account_keys: &[Pubkey],
) -> Result<ParsedInstructionEnum, ParseInstructionError> {
    match instruction.accounts.iter().max() {
        Some(index) if (*index as usize) < account_keys.len() => {}
        _ => {
            // Runtime should prevent this from ever happening
            return Err(ParseInstructionError::InstructionKeyMismatch(
                ParsableProgram::SPLAssociatedTokenAccount,
            ));
        }
    }
    check_num_associated_token_accounts(&instruction.accounts, 7)?;
    Ok(ParsedInstructionEnum {
        instruction_type: "Create".to_string(),
        info: json!({
            "Source": account_keys[instruction.accounts[0] as usize].to_string(),
            "Account": account_keys[instruction.accounts[1] as usize].to_string(),
            "Wallet": account_keys[instruction.accounts[2] as usize].to_string(),
            "Mint": account_keys[instruction.accounts[3] as usize].to_string(),
            "System Program": account_keys[instruction.accounts[4] as usize].to_string(),
            "Token Program": account_keys[instruction.accounts[5] as usize].to_string(),
            "Rent Sysvar": account_keys[instruction.accounts[6] as usize].to_string(),
        }),
    })
}

fn check_num_associated_token_accounts(
    accounts: &[u8],
    num: usize,
) -> Result<(), ParseInstructionError> {
    check_num_accounts(accounts, num, ParsableProgram::SPLAssociatedTokenAccount)
}

#[cfg(test)]
mod test {
    use super::*;
    use solana_account_decoder::parse_token::pubkey_from_spl_token;
    #[allow(deprecated)]
    use spl_associated_token_account::create_associated_token_account as create_associated_token_account_deprecated;
    use spl_associated_token_account::{
        get_associated_token_address,
        solana_program::{
            instruction::CompiledInstruction as SplAssociatedTokenCompiledInstruction,
            message::Message, pubkey::Pubkey as SplAssociatedTokenPubkey, sysvar,
        },
    };

    fn convert_pubkey(pubkey: Pubkey) -> SplAssociatedTokenPubkey {
        SplAssociatedTokenPubkey::new_from_array(pubkey.to_bytes())
    }

    fn convert_compiled_instruction(
        instruction: &SplAssociatedTokenCompiledInstruction,
    ) -> CompiledInstruction {
        CompiledInstruction {
            program_id_index: instruction.program_id_index,
            accounts: instruction.accounts.clone(),
            data: instruction.data.clone(),
        }
    }

    fn convert_account_keys(message: &Message) -> Vec<Pubkey> {
        message
            .account_keys
            .iter()
            .map(pubkey_from_spl_token)
            .collect()
    }

    #[test]
    fn test_parse_associated_token_deprecated() {
        let funder = Pubkey::new_unique();
        let wallet_address = Pubkey::new_unique();
        let mint = Pubkey::new_unique();
        let associated_account_address =
            get_associated_token_address(&convert_pubkey(wallet_address), &convert_pubkey(mint));
        #[allow(deprecated)]
        let create_ix = create_associated_token_account_deprecated(
            &convert_pubkey(funder),
            &convert_pubkey(wallet_address),
            &convert_pubkey(mint),
        );
        let message = Message::new(&[create_ix], None);
        let compiled_instruction = convert_compiled_instruction(&message.instructions[0]);
        assert_eq!(
            parse_associated_token_account(&compiled_instruction, &convert_account_keys(&message))
                .unwrap(),
            ParsedInstructionEnum {
                instruction_type: "Create".to_string(),
                info: json!({
                    "Source": funder.to_string(),
                    "Account": associated_account_address.to_string(),
                    "Wallet": wallet_address.to_string(),
                    "Mint": mint.to_string(),
                    "System Program": solana_sdk::system_program::id().to_string(),
                    "Token Program": spl_token::id().to_string(),
                    "Rent Sysvar": sysvar::rent::id().to_string(),
                })
            }
        );
    }
}
