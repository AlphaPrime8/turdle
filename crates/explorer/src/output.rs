use crate::{
    account::{AccountFieldVisibility, DisplayKeyedAccount, KeyedAccount},
    config::ExplorerConfig,
    display::DisplayFormat,
    error::{ExplorerError, Result},
    program::{DisplayUpgradeableProgram, ProgramFieldVisibility},
    transaction::{
        DisplayRawTransaction, DisplayTransaction, RawTransactionFieldVisibility,
        TransactionFieldVisibility,
    },
};
use console::style;
use pretty_hex::*;
use solana_client::rpc_config::RpcTransactionConfig;
use solana_sdk::{
    account_utils::StateMut, bpf_loader, bpf_loader_deprecated, bpf_loader_upgradeable,
    bpf_loader_upgradeable::UpgradeableLoaderState, commitment_config::CommitmentConfig,
    native_token, pubkey::Pubkey, signature::Signature,
};
use solana_transaction_status::{TransactionConfirmationStatus, UiTransactionEncoding};
use std::{cmp::Ordering, fmt::Write};

pub fn pretty_lamports_to_sol(lamports: u64) -> String {
    let sol_str = format!("{:.9}", native_token::lamports_to_sol(lamports));
    sol_str
        .trim_end_matches('0')
        .trim_end_matches('.')
        .to_string()
}

pub fn classify_account(fee_payer: bool, writable: bool, signer: bool, program: bool) -> String {
    let mut account_type_string = String::new();
    let mut started = false;
    if fee_payer {
        account_type_string.push_str("[Fee Payer]");
        started = true;
    }
    if writable {
        if started {
            account_type_string.push(' ');
        }
        account_type_string.push_str("[Writable]");
        started = true;
    }
    if signer {
        if started {
            account_type_string.push(' ');
        }
        account_type_string.push_str("[Signer]");
        started = true;
    }
    if program {
        if started {
            account_type_string.push(' ');
        }
        account_type_string.push_str("[Program]");
    }
    account_type_string
}

pub fn calculate_change(post: u64, pre: u64) -> String {
    match post.cmp(&pre) {
        Ordering::Greater => format!(
            "◎ {} (+{})",
            pretty_lamports_to_sol(post),
            pretty_lamports_to_sol(post - pre)
        ),
        Ordering::Less => format!(
            "◎ {} (-{})",
            pretty_lamports_to_sol(post),
            pretty_lamports_to_sol(pre - post)
        ),
        Ordering::Equal => format!("◎ {}", pretty_lamports_to_sol(post)),
    }
}

pub fn change_in_sol(post: u64, pre: u64) -> String {
    match post.cmp(&pre) {
        Ordering::Greater => format!("+{}", pretty_lamports_to_sol(post - pre)),
        Ordering::Less => format!("-{}", pretty_lamports_to_sol(pre - post)),
        Ordering::Equal => "0".to_string(),
    }
}

pub fn status_to_string(status: &TransactionConfirmationStatus) -> String {
    match status {
        TransactionConfirmationStatus::Processed => "Processed".to_string(),
        TransactionConfirmationStatus::Confirmed => "Confirmed".to_string(),
        TransactionConfirmationStatus::Finalized => "Finalized".to_string(),
    }
}

pub async fn print_account(
    pubkey: &Pubkey,
    visibility: &AccountFieldVisibility,
    format: DisplayFormat,
    config: &ExplorerConfig,
) -> Result<()> {
    let account_string = get_account_string(pubkey, visibility, format, config).await?;
    println!("{account_string}");
    Ok(())
}

pub async fn print_program(
    program_id: &Pubkey,
    visibility: &ProgramFieldVisibility,
    format: DisplayFormat,
    config: &ExplorerConfig,
) -> Result<()> {
    let program_string = get_program_string(program_id, visibility, format, config).await?;
    println!("{program_string}");
    Ok(())
}

pub async fn print_raw_transaction(
    signature: &Signature,
    visibility: &RawTransactionFieldVisibility,
    format: DisplayFormat,
    config: &ExplorerConfig,
) -> Result<()> {
    let raw_transaction_string =
        get_raw_transaction_string(signature, visibility, format, config).await?;
    println!("{raw_transaction_string}");
    Ok(())
}

pub async fn print_transaction(
    signature: &Signature,
    visibility: &TransactionFieldVisibility,
    format: DisplayFormat,
    config: &ExplorerConfig,
) -> Result<()> {
    let transaction_string = get_transaction_string(signature, visibility, format, config).await?;
    println!("{transaction_string}");
    Ok(())
}

pub async fn get_account_string(
    pubkey: &Pubkey,
    visibility: &AccountFieldVisibility,
    format: DisplayFormat,
    config: &ExplorerConfig,
) -> Result<String> {
    let rpc_client = config.rpc_client();
    let account = rpc_client.get_account(pubkey)?;
    let keyed_account = KeyedAccount {
        pubkey: *pubkey,
        account,
    };
    let display_keyed_account = DisplayKeyedAccount::from_keyed_account(&keyed_account, visibility);
    let mut account_string = format.formatted_string(&display_keyed_account)?;

    if display_keyed_account.account.data.is_some() {
        let data = &keyed_account.account.data;
        if let DisplayFormat::Cli = format {
            if !data.is_empty() {
                writeln!(&mut account_string)?;
                writeln!(&mut account_string)?;

                writeln!(
                    &mut account_string,
                    "{} {} bytes",
                    style("Hexdump:").bold(),
                    data.len()
                )?;
                // Show hexdump of not more than MAX_BYTES_SHOWN bytes
                const MAX_BYTES_SHOWN: usize = 64;
                let len = data.len();
                let (end, finished) = if MAX_BYTES_SHOWN > len {
                    (len, true)
                } else {
                    (MAX_BYTES_SHOWN, false)
                };
                let raw_account_data = &data[..end];
                let cfg = HexConfig {
                    title: false,
                    width: 16,
                    group: 0,
                    chunk: 2,
                    ..HexConfig::default()
                };
                write!(&mut account_string, "{:?}", raw_account_data.hex_conf(cfg))?;
                if !finished {
                    writeln!(&mut account_string)?;
                    write!(&mut account_string, "... (skipped)")?;
                }
            }
        };
    }

    Ok(account_string)
}

pub async fn get_program_string(
    program_id: &Pubkey,
    visibility: &ProgramFieldVisibility,
    format: DisplayFormat,
    config: &ExplorerConfig,
) -> Result<String> {
    let rpc_client = config.rpc_client();
    let program_account = rpc_client.get_account(program_id)?;
    let program_keyed_account = KeyedAccount {
        pubkey: *program_id,
        account: program_account,
    };

    if program_keyed_account.account.owner == bpf_loader::id()
        || program_keyed_account.account.owner == bpf_loader_deprecated::id()
    {
        // these loaders are not interesting, just accounts with the program.so in data
        let mut program_string = get_account_string(
            program_id,
            &AccountFieldVisibility::new_all_enabled(),
            format,
            config,
        )
        .await?;

        if let DisplayFormat::Cli = format {
            program_string.push_str(
                "\n\nNote: the program is loaded either by the deprecated BPFLoader or BPFLoader2,
it is an executable account with program.so in its data, hence this output.",
            );
        }

        Ok(program_string)
    } else if program_keyed_account.account.owner == bpf_loader_upgradeable::id() {
        // this is the only interesting loader which uses redirection to programdata account
        if let Ok(UpgradeableLoaderState::Program {
            programdata_address,
        }) = program_keyed_account.account.state()
        {
            if let Ok(programdata_account) = rpc_client.get_account(&programdata_address) {
                let programdata_keyed_account = KeyedAccount {
                    pubkey: programdata_address,
                    account: programdata_account,
                };
                if let Ok(UpgradeableLoaderState::ProgramData {
                    upgrade_authority_address,
                    slot,
                }) = programdata_keyed_account.account.state()
                {
                    let program = DisplayUpgradeableProgram::from(
                        &program_keyed_account,
                        &programdata_keyed_account,
                        slot,
                        &upgrade_authority_address,
                        visibility,
                    );
                    let mut program_string = format.formatted_string(&program)?;

                    if program.programdata_account.is_some() {
                        if let DisplayFormat::Cli = format {
                            writeln!(&mut program_string)?;
                            writeln!(&mut program_string)?;
                            writeln!(
                                &mut program_string,
                                "{} {} bytes",
                                style("Followed by Raw Program Data (program.so):").bold(),
                                programdata_keyed_account.account.data.len()
                                    - UpgradeableLoaderState::size_of_programdata_metadata()
                            )?;

                            // Show hexdump of not more than MAX_BYTES_SHOWN bytes
                            const MAX_BYTES_SHOWN: usize = 64;
                            let len = programdata_keyed_account.account.data.len();
                            let offset = UpgradeableLoaderState::size_of_programdata_metadata();
                            let (end, finished) = if offset + MAX_BYTES_SHOWN > len {
                                (len, true)
                            } else {
                                (offset + MAX_BYTES_SHOWN, false)
                            };
                            let raw_program_data =
                                &programdata_keyed_account.account.data[offset..end];
                            let cfg = HexConfig {
                                title: false,
                                width: 16,
                                group: 0,
                                chunk: 2,
                                ..HexConfig::default()
                            };
                            write!(&mut program_string, "{:?}", raw_program_data.hex_conf(cfg))?;
                            if !finished {
                                writeln!(&mut program_string)?;
                                write!(&mut program_string, "... (skipped)")?;
                            }
                        }
                    }

                    Ok(program_string)
                } else {
                    Err(ExplorerError::Custom(format!(
                        "Program {program_id} has been closed"
                    )))
                }
            } else {
                Err(ExplorerError::Custom(format!(
                    "Program {program_id} has been closed"
                )))
            }
        } else {
            Err(ExplorerError::Custom(format!(
                "{program_id} is not a Program account"
            )))
        }
    } else {
        Err(ExplorerError::Custom(format!(
            "{program_id} is not a pubkey of an on-chain BPF program."
        )))
    }
}

pub async fn get_raw_transaction_string(
    signature: &Signature,
    visibility: &RawTransactionFieldVisibility,
    format: DisplayFormat,
    config: &ExplorerConfig,
) -> Result<String> {
    let rpc_client = config.rpc_client();
    let config = RpcTransactionConfig {
        encoding: Some(UiTransactionEncoding::Binary),
        commitment: Some(CommitmentConfig::confirmed()),
        max_supported_transaction_version: None,
    };

    let transaction = rpc_client.get_transaction_with_config(signature, config)?;

    let response = rpc_client.get_signature_statuses_with_history(&[*signature])?;

    let transaction_status = response.value[0].as_ref().unwrap();

    let display_transaction =
        DisplayRawTransaction::from(&transaction, transaction_status, visibility)?;

    let transaction_string = format.formatted_string(&display_transaction)?;

    Ok(transaction_string)
}

pub async fn get_transaction_string(
    signature: &Signature,
    visibility: &TransactionFieldVisibility,
    format: DisplayFormat,
    config: &ExplorerConfig,
) -> Result<String> {
    let rpc_client = config.rpc_client();
    let config = RpcTransactionConfig {
        encoding: Some(UiTransactionEncoding::Binary),
        commitment: Some(CommitmentConfig::confirmed()),
        max_supported_transaction_version: None,
    };

    let transaction = rpc_client.get_transaction_with_config(signature, config)?;

    let response = rpc_client.get_signature_statuses_with_history(&[*signature])?;

    let transaction_status = response.value[0].as_ref().unwrap();

    let display_transaction =
        DisplayTransaction::from(&transaction, transaction_status, visibility)?;

    let transaction_string = format.formatted_string(&display_transaction)?;

    Ok(transaction_string)
}
