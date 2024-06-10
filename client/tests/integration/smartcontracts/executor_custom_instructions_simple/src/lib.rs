//! Runtime Executor which extends instruction set with one custom instruction - [MintAssetForAllAccounts].
//! This instruction is handled in executor, and translates to multiple usual ISIs.
//! It is possible to use queries during execution.

#![no_std]

extern crate alloc;
#[cfg(not(test))]
extern crate panic_halt;

use executor_custom_data_model::simple::{CustomInstructionBox, MintAssetForAllAccounts};
use iroha_executor::{data_model::isi::Custom, debug::DebugExpectExt, prelude::*};
use lol_alloc::{FreeListAllocator, LockedAllocator};

#[global_allocator]
static ALLOC: LockedAllocator<FreeListAllocator> = LockedAllocator::new(FreeListAllocator::new());

getrandom::register_custom_getrandom!(iroha_executor::stub_getrandom);

#[derive(Constructor, ValidateEntrypoints, Validate, Visit)]
#[visit(custom(visit_custom))]
struct Executor {
    verdict: Result,
    block_height: u64,
}

fn visit_custom(executor: &mut Executor, _authority: &AccountId, isi: &Custom) {
    let Ok(isi) = CustomInstructionBox::try_from(isi.payload()) else {
        deny!(executor, "Failed to parse custom instruction");
    };
    match execute_custom_instruction(isi) {
        Ok(()) => return,
        Err(err) => {
            deny!(executor, err);
        }
    }
}

fn execute_custom_instruction(isi: CustomInstructionBox) -> Result<(), ValidationFail> {
    match isi {
        CustomInstructionBox::MintAssetForAllAccounts(isi) => {
            execute_mint_asset_for_all_accounts(isi)
        }
    }
}

fn execute_mint_asset_for_all_accounts(isi: MintAssetForAllAccounts) -> Result<(), ValidationFail> {
    let accounts = FindAccountsWithAsset::new(isi.asset_definition.clone()).execute()?;
    for account in accounts {
        let account = account.dbg_expect("Failed to get accounts with asset");
        let asset_id = AssetId::new(isi.asset_definition.clone(), account.id().clone());
        Mint::asset_numeric(isi.quantity, asset_id).execute()?;
    }
    Ok(())
}

#[entrypoint]
pub fn migrate(_block_height: u64) -> MigrationResult {
    DataModelBuilder::with_default_permissions()
        .with_custom_instruction::<CustomInstructionBox>()
        .build_and_set();

    Ok(())
}
