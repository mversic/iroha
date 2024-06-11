//! trigger which mints one rose or its owner.

#![no_std]

#[cfg(not(test))]
extern crate panic_halt;

use iroha_trigger::prelude::*;
use lol_alloc::{FreeListAllocator, LockedAllocator};

#[global_allocator]
static ALLOC: LockedAllocator<FreeListAllocator> = LockedAllocator::new(FreeListAllocator::new());

getrandom::register_custom_getrandom!(iroha_trigger::stub_getrandom);

/// Mint 1 rose for owner
#[iroha_trigger::main]
fn main(id: TriggerId, owner: AccountId, _event: EventBox) {
    let rose_definition_id = "rose#wonderland".parse()
        .dbg_expect("Failed to parse `rose#wonderland` asset definition id");
    let rose_id = AssetId::new(rose_definition_id, owner);

    let val: u32 = FindTriggerKeyValueByIdAndKey::new(id, "VAL".parse().unwrap())
        .execute()
        .dbg_unwrap()
        .into_inner()
        .try_into()
        .dbg_unwrap();

    Mint::asset_numeric(val, rose_id)
        .execute()
        .dbg_expect("Failed to mint rose");
}
