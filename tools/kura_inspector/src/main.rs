//! Kura inspector binary. For usage run with `--help`.
#![allow(
    clippy::arithmetic_side_effects,
    clippy::std_instead_of_core,
    clippy::std_instead_of_alloc
)]
use std::path::{Path, PathBuf};

use clap::{Parser, Subcommand};
use iroha_core::kura::{BlockIndex, BlockStore, LockStatus};
use iroha_data_model::block::VersionedSignedBlock;
use iroha_version::scale::DecodeVersioned;

/// Kura inspector
#[derive(Parser)]
#[clap(author, version, about)]
struct Args {
    /// Height of the block from which start the inspection.
    /// Defaults to the latest block height
    #[clap(short, long, name = "BLOCK_HEIGHT")]
    from: Option<u64>,
    #[clap()]
    path_to_block_store: PathBuf,
    #[clap(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Print contents of a certain length of the blocks
    Print {
        /// Number of the blocks to print.
        /// The excess will be truncated
        #[clap(short = 'n', long, default_value_t = 1)]
        length: u64,
    },
}

#[allow(clippy::use_debug, clippy::print_stderr, clippy::panic)]
fn main() {
    let args = Args::parse();

    let from_height = args.from.map(|height| {
        assert!(height != 0, "The genesis block has the height 1. Therefore, the \"from height\" you specify must not be 0.");
        // Kura starts counting blocks from 0 like an array while the outside world counts the first block as number 1.
        height - 1
    });

    match args.command {
        Command::Print { length } => print_blockchain(
            &args.path_to_block_store,
            from_height.unwrap_or(u64::MAX),
            length,
        ),
    }
}

#[allow(
    clippy::print_stdout,
    clippy::use_debug,
    clippy::expect_used,
    clippy::expect_fun_call
)]
fn print_blockchain(block_store_path: &Path, from_height: u64, block_count: u64) {
    let mut block_store_path: std::borrow::Cow<'_, Path> = block_store_path.into();

    if let Some(os_str_file_name) = block_store_path.file_name() {
        let file_name_str = os_str_file_name.to_str().unwrap_or("");
        if file_name_str == "blocks.data" || file_name_str == "blocks.index" {
            block_store_path.to_mut().pop();
        }
    }

    let block_store = BlockStore::new(&block_store_path, LockStatus::Unlocked);

    let index_count = block_store
        .read_index_count()
        .expect("Failed to read index count from block store {block_store_path:?}.");

    if index_count == 0 {
        println!("The block store is empty.");
        return;
    }

    assert!(
        index_count != 0,
        "Index count is zero. This could be because there are no blocks in the store: {block_store_path:?}"
    );

    let from_height = if from_height >= index_count {
        index_count - 1
    } else {
        from_height
    };

    let block_count = if from_height + block_count > index_count {
        index_count - from_height
    } else {
        block_count
    };

    let mut block_indices = vec![
        BlockIndex {
            start: 0,
            length: 0
        };
        block_count
            .try_into()
            .expect("block_count didn't fit in 32-bits")
    ];
    block_store
        .read_block_indices(from_height, &mut block_indices)
        .expect("Failed to read block indices");
    let block_indices = block_indices;

    // Now for the actual printing
    println!("Index file says there are {index_count} blocks.");
    println!(
        "Printing blocks {}-{}...",
        from_height + 1,
        from_height + block_count
    );

    for i in 0..block_count {
        let idx = block_indices[usize::try_from(i).expect("i didn't fit in 32-bits")];
        let meta_index = from_height + i;

        println!(
            "Block#{} starts at byte offset {} and is {} bytes long.",
            meta_index + 1,
            idx.start,
            idx.length
        );
        let mut block_buf =
            vec![0_u8; usize::try_from(idx.length).expect("index_len didn't fit in 32-bits")];
        block_store
            .read_block_data(idx.start, &mut block_buf)
            .expect(&format!("Failed to read block № {} data.", meta_index + 1));
        let block = VersionedSignedBlock::decode_all_versioned(&block_buf)
            .expect(&format!("Failed to decode block № {}", meta_index + 1));
        println!("Block#{} :", meta_index + 1);
        println!("{block:#?}");
    }
}
