//! This module contains `Block` structures for each state, it's
//! transitions, implementations and related traits
//! implementations. `Block`s are organised into a linear sequence
//! over time (also known as the block chain).  A Block's life-cycle
//! starts from `PendingBlock`.

#[cfg(not(feature = "std"))]
use alloc::{boxed::Box, format, string::String, vec::Vec};
use core::{cmp::Ordering, fmt::Display, time::Duration};

use derive_more::Display;
use getset::Getters;
use iroha_crypto::{HashOf, KeyPair, MerkleTree, SignaturesOf};
use iroha_data_model_derive::model;
use iroha_macro::FromVariant;
use iroha_schema::IntoSchema;
use iroha_version::{declare_versioned, version_with_scale};
use parity_scale_codec::{Decode, Encode};
use serde::{Deserialize, Serialize};

pub use self::model::*;
use crate::{events::prelude::*, peer, transaction::prelude::*};

#[model]
pub mod model {
    use super::*;

    #[derive(
        Debug,
        Display,
        Clone,
        PartialEq,
        Eq,
        PartialOrd,
        Ord,
        Getters,
        Decode,
        Encode,
        Deserialize,
        Serialize,
        IntoSchema,
    )]
    #[cfg_attr(
        feature = "std",
        display(fmt = "Block №{height} (hash: {});", "HashOf::new(&self)")
    )]
    #[cfg_attr(not(feature = "std"), display(fmt = "Block №{height}"))]
    #[getset(get = "pub")]
    #[allow(missing_docs)]
    #[ffi_type]
    // TODO: Do we need both BlockPayload and BlockHeader?
    // If yes, what data goes into which structure?
    pub struct BlockHeader {
        /// Number of blocks in the chain including this block.
        pub height: u64,
        /// Creation timestamp (unix time in milliseconds).
        #[getset(skip)]
        pub timestamp_ms: u64,
        /// Hash of the previous block in the chain.
        pub previous_block_hash: Option<HashOf<VersionedSignedBlock>>,
        /// Hash of merkle tree root of transactions' hashes.
        pub transactions_hash: Option<HashOf<MerkleTree<VersionedSignedTransaction>>>,
        /// Topology of the network at the time of block commit.
        #[getset(skip)] // FIXME: Because ffi related issues
        pub commit_topology: Vec<peer::PeerId>,
        /// Value of view change index. Used to resolve soft forks.
        pub view_change_index: u64,
        /// Estimation of consensus duration (in milliseconds).
        pub consensus_estimation_ms: u64,
    }

    #[derive(
        Debug, Display, Clone, Eq, Getters, Decode, Encode, Deserialize, Serialize, IntoSchema,
    )]
    #[display(fmt = "({header})")]
    #[getset(get = "pub")]
    #[allow(missing_docs)]
    #[ffi_type]
    pub struct BlockPayload {
        /// Block header
        pub header: BlockHeader,
        /// array of transactions, which successfully passed validation and consensus step.
        #[getset(skip)] // FIXME: Because ffi related issues
        pub transactions: Vec<TransactionValue>,
        /// Event recommendations.
        #[getset(skip)] // NOTE: Unused ATM
        pub event_recommendations: Vec<Event>,
    }

    /// Signed block
    #[version_with_scale(version = 1, versioned_alias = "VersionedSignedBlock")]
    #[derive(
        Debug,
        Display,
        Clone,
        PartialEq,
        Eq,
        PartialOrd,
        Ord,
        Getters,
        Encode,
        Serialize,
        IntoSchema,
    )]
    #[cfg_attr(not(feature = "std"), display(fmt = "Signed block"))]
    #[cfg_attr(feature = "std", display(fmt = "{}", "self.hash()"))]
    #[getset(get = "pub")]
    #[ffi_type]
    pub struct SignedBlock {
        /// Signatures of peers which approved this block.
        #[getset(skip)]
        pub signatures: SignaturesOf<BlockPayload>,
        /// Block payload
        pub payload: BlockPayload,
    }
}

#[cfg(any(feature = "ffi_export", feature = "ffi_import"))]
declare_versioned!(VersionedSignedBlock 1..2, Debug, Clone, PartialEq, Eq, PartialOrd, Ord, FromVariant, iroha_ffi::FfiType, IntoSchema);
#[cfg(all(not(feature = "ffi_export"), not(feature = "ffi_import")))]
declare_versioned!(VersionedSignedBlock 1..2, Debug, Clone, PartialEq, Eq, PartialOrd, Ord, FromVariant, IntoSchema);

// TODO: Think about how should BlockPayload implement Eq, Ord?
impl PartialEq for BlockPayload {
    fn eq(&self, other: &Self) -> bool {
        self.header == other.header
    }
}
impl PartialOrd for BlockPayload {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for BlockPayload {
    fn cmp(&self, other: &Self) -> Ordering {
        self.header.cmp(&other.header)
    }
}

impl BlockPayload {
    /// Calculate block payload [`Hash`](`iroha_crypto::HashOf`).
    #[cfg(feature = "std")]
    pub fn hash(&self) -> iroha_crypto::HashOf<Self> {
        iroha_crypto::HashOf::new(self)
    }
}

impl BlockHeader {
    /// Checks if it's a header of a genesis block.
    #[inline]
    pub const fn is_genesis(&self) -> bool {
        self.height == 1
    }

    /// Creation timestamp
    pub fn timestamp(&self) -> Duration {
        Duration::from_millis(self.timestamp_ms)
    }

    /// Consensus estimation
    pub fn consensus_estimation(&self) -> Duration {
        Duration::from_millis(self.consensus_estimation_ms)
    }
}

impl SignedBlock {
    #[cfg(feature = "std")]
    fn hash(&self) -> iroha_crypto::HashOf<VersionedSignedBlock> {
        iroha_crypto::HashOf::from_untyped_unchecked(iroha_crypto::HashOf::new(self).into())
    }
}

impl VersionedSignedBlock {
    /// Block payload
    // FIXME: Leaking concrete type BlockPayload from Versioned container. Payload should be versioned
    pub fn payload(&self) -> &BlockPayload {
        let VersionedSignedBlock::V1(block) = self;
        block.payload()
    }

    /// Used to inject faulty payload for testing
    #[cfg(debug_assertions)]
    #[cfg(feature = "transparent_api")]
    pub fn payload_mut(&mut self) -> &mut BlockPayload {
        let VersionedSignedBlock::V1(block) = self;
        &mut block.payload
    }

    /// Signatures of peers which approved this block.
    pub fn signatures(&self) -> &SignaturesOf<BlockPayload> {
        let VersionedSignedBlock::V1(block) = self;
        &block.signatures
    }

    /// Calculate block hash
    #[cfg(feature = "std")]
    pub fn hash(&self) -> HashOf<Self> {
        iroha_crypto::HashOf::new(self)
    }

    /// Add additional signatures to this block
    ///
    /// # Errors
    ///
    /// If given signature doesn't match block hash
    #[cfg(feature = "std")]
    #[cfg(feature = "transparent_api")]
    pub fn sign(mut self, key_pair: KeyPair) -> Result<Self, iroha_crypto::error::Error> {
        iroha_crypto::SignatureOf::new(key_pair, self.payload()).map(|signature| {
            let VersionedSignedBlock::V1(block) = &mut self;
            block.signatures.insert(signature);
            self
        })
    }

    /// Add additional signatures to this block
    ///
    /// # Errors
    ///
    /// If given signature doesn't match block hash
    #[cfg(feature = "std")]
    #[cfg(feature = "transparent_api")]
    pub fn add_signature(
        &mut self,
        signature: iroha_crypto::SignatureOf<BlockPayload>,
    ) -> Result<(), iroha_crypto::error::Error> {
        signature.verify(self.payload())?;

        let VersionedSignedBlock::V1(block) = self;
        block.signatures.insert(signature);

        Ok(())
    }

    /// Add additional signatures to this block
    #[cfg(feature = "std")]
    #[cfg(feature = "transparent_api")]
    pub fn replace_signatures(
        &mut self,
        signatures: iroha_crypto::SignaturesOf<BlockPayload>,
    ) -> bool {
        #[cfg(not(feature = "std"))]
        use alloc::collections::BTreeSet;
        #[cfg(feature = "std")]
        use std::collections::BTreeSet;

        let VersionedSignedBlock::V1(block) = self;
        block.signatures = BTreeSet::new().into();

        for signature in signatures {
            if self.add_signature(signature).is_err() {
                return false;
            }
        }

        true
    }
}

mod candidate {
    use parity_scale_codec::Input;

    use super::*;

    #[derive(Decode, Deserialize)]
    struct SignedBlockCandidate {
        signatures: SignaturesOf<BlockPayload>,
        payload: BlockPayload,
    }

    impl SignedBlockCandidate {
        fn validate(self) -> Result<SignedBlock, &'static str> {
            #[cfg(feature = "std")]
            self.validate_signatures()?;
            #[cfg(feature = "std")]
            self.validate_header()?;

            if self.payload.transactions.is_empty() {
                return Err("Block is empty");
            }

            Ok(SignedBlock {
                payload: self.payload,
                signatures: self.signatures,
            })
        }

        #[cfg(feature = "std")]
        fn validate_header(&self) -> Result<(), &'static str> {
            let actual_txs_hash = self.payload.header().transactions_hash;

            let expected_txs_hash = self
                .payload
                .transactions
                .iter()
                .map(TransactionValue::hash)
                .collect::<MerkleTree<_>>()
                .hash();

            if expected_txs_hash != actual_txs_hash {
                return Err("Transactions' hash incorrect. Expected: {expected_txs_hash:?}, actual: {actual_txs_hash:?}");
            }
            // TODO: Validate Event recommendations somehow?

            Ok(())
        }

        #[cfg(feature = "std")]
        fn validate_signatures(&self) -> Result<(), &'static str> {
            self.signatures
                .verify(&self.payload)
                .map_err(|_| "Transaction contains invalid signatures")
        }
    }

    impl Decode for SignedBlock {
        fn decode<I: Input>(input: &mut I) -> Result<Self, parity_scale_codec::Error> {
            SignedBlockCandidate::decode(input)?
                .validate()
                .map_err(Into::into)
        }
    }
    impl<'de> Deserialize<'de> for SignedBlock {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: serde::Deserializer<'de>,
        {
            use serde::de::Error as _;

            SignedBlockCandidate::deserialize(deserializer)?
                .validate()
                .map_err(D::Error::custom)
        }
    }
}

impl Display for VersionedSignedBlock {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let VersionedSignedBlock::V1(block) = self;
        block.fmt(f)
    }
}

#[cfg(feature = "http")]
pub mod stream {
    //! Blocks for streaming API.

    use derive_more::Constructor;
    use iroha_schema::IntoSchema;
    use parity_scale_codec::{Decode, Encode};

    pub use self::model::*;
    use super::*;

    #[model]
    pub mod model {
        use core::num::NonZeroU64;

        use super::*;

        /// Request sent to subscribe to blocks stream starting from the given height.
        #[derive(Debug, Clone, Copy, Constructor, Decode, Encode, IntoSchema)]
        #[repr(transparent)]
        pub struct BlockSubscriptionRequest(pub NonZeroU64);

        /// Message sent by the stream producer containing block.
        #[derive(Debug, Clone, Decode, Encode, IntoSchema)]
        #[repr(transparent)]
        pub struct BlockMessage(pub VersionedSignedBlock);
    }

    impl From<BlockMessage> for VersionedSignedBlock {
        fn from(source: BlockMessage) -> Self {
            source.0
        }
    }

    /// Exports common structs and enums from this module.
    pub mod prelude {
        pub use super::{BlockMessage, BlockSubscriptionRequest};
    }
}

pub mod error {
    //! Module containing errors that can occur during instruction evaluation

    pub use self::model::*;
    use super::*;

    #[model]
    pub mod model {
        use super::*;

        /// The reason for rejecting a transaction with new blocks.
        #[derive(
            Debug,
            Display,
            Clone,
            Copy,
            PartialEq,
            Eq,
            iroha_macro::FromVariant,
            Decode,
            Encode,
            Deserialize,
            Serialize,
            IntoSchema,
        )]
        #[display(fmt = "Block was rejected during consensus")]
        #[serde(untagged)] // Unaffected by #3330 as it's a unit variant
        #[repr(transparent)]
        #[ffi_type]
        pub enum BlockRejectionReason {
            /// Block was rejected during consensus.
            ConsensusBlockRejection,
        }
    }

    #[cfg(feature = "std")]
    impl std::error::Error for BlockRejectionReason {}
}
