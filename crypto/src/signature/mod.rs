// pub(crate) for inner modules it is not redundant, the contents of `signature` module get re-exported at root
#![allow(clippy::redundant_pub_crate)]

#[cfg(not(feature = "ffi_import"))]
pub(crate) mod bls;

#[cfg(not(feature = "ffi_import"))]
pub(crate) mod ed25519;

#[cfg(not(feature = "ffi_import"))]
pub(crate) mod secp256k1;

#[cfg(not(feature = "std"))]
use alloc::{
    boxed::Box, collections::btree_set, format, string::String, string::ToString as _, vec,
    vec::Vec,
};
use core::{borrow::Borrow as _, marker::PhantomData};
#[cfg(feature = "std")]
use std::collections::btree_set;

use arrayref::array_ref;
use derive_more::{Deref, DerefMut};
use iroha_primitives::const_vec::ConstVec;
use iroha_schema::{IntoSchema, TypeId};
use parity_scale_codec::{Decode, Encode};
use rand_core::{CryptoRngCore, SeedableRng as _};
#[cfg(not(feature = "ffi_import"))]
use serde::{Deserialize, Serialize};
use sha2::Digest as _;
use zeroize::Zeroize as _;

use crate::{error::ParseError, ffi, hex_decode, Error, HashOf, KeyPair, PublicKey};

/// Construct cryptographic RNG from seed.
fn rng_from_seed(mut seed: Vec<u8>) -> impl CryptoRngCore {
    let hash = sha2::Sha256::digest(&seed);
    seed.zeroize();
    rand_chacha::ChaChaRng::from_seed(*array_ref!(hash.as_slice(), 0, 32))
}

ffi::ffi_item! {
    /// Represents a signature of the data (`Block` or `Transaction` for example).
    #[serde_with::serde_as]
    #[derive(Clone, PartialEq, Eq, PartialOrd, Ord, getset::Getters)]
    #[cfg_attr(not(feature="ffi_import"), derive(derive_more::DebugCustom, Hash, Decode, Encode, Deserialize, Serialize, IntoSchema))]
    #[cfg_attr(not(feature="ffi_import"), debug(
        fmt = "{{ pub_key: {public_key}, payload: {} }}",
        "hex::encode_upper(payload)"
    ))]
    pub struct Signature {
        /// Public key that is used for verification. Payload is verified by algorithm
        /// that corresponds with the public key's digest function.
        #[getset(get = "pub")]
        public_key: PublicKey,
        /// Signature payload
        #[serde_as(as = "serde_with::hex::Hex<serde_with::formats::Uppercase>")]
        payload: ConstVec<u8>,
    }
}

impl Signature {
    /// Access the signature's payload
    pub fn payload(&self) -> &[u8] {
        self.payload.as_ref()
    }

    /// Creates new signature by signing payload via [`KeyPair::private_key`].
    pub fn new(key_pair: &KeyPair, payload: &[u8]) -> Self {
        use crate::secrecy::ExposeSecret;
        let signature = match key_pair.private_key.0.expose_secret() {
            crate::PrivateKeyInner::Ed25519(sk) => ed25519::Ed25519Sha512::sign(payload, sk),
            crate::PrivateKeyInner::Secp256k1(sk) => {
                secp256k1::EcdsaSecp256k1Sha256::sign(payload, sk)
            }
            crate::PrivateKeyInner::BlsSmall(sk) => bls::BlsSmall::sign(payload, sk),
            crate::PrivateKeyInner::BlsNormal(sk) => bls::BlsNormal::sign(payload, sk),
        };
        Self {
            public_key: key_pair.public_key.clone(),
            payload: ConstVec::new(signature),
        }
    }

    /// Creates new signature from its raw payload and public key.
    ///
    /// **This method does not sign the payload.** Use [`Signature::new`] for this purpose.
    ///
    /// This method exists to allow reproducing the signature in a more efficient way than through
    /// deserialization.
    pub fn from_bytes(public_key: PublicKey, payload: &[u8]) -> Self {
        Self {
            public_key,
            payload: ConstVec::new(payload),
        }
    }

    /// A shorthand for [`Self::from_bytes`] accepting payload as hex.
    ///
    /// # Errors
    /// If passed string is not a valid hex.
    pub fn from_hex(public_key: PublicKey, payload: impl AsRef<str>) -> Result<Self, ParseError> {
        let payload: Vec<u8> = hex_decode(payload.as_ref())?;
        Ok(Self::from_bytes(public_key, &payload))
    }

    /// Verify `payload` using signed data and [`KeyPair::public_key`].
    ///
    /// # Errors
    /// Fails if the message doesn't pass verification
    pub fn verify(&self, payload: &[u8]) -> Result<(), Error> {
        match self.public_key.0.borrow() {
            crate::PublicKeyInner::Ed25519(pk) => {
                ed25519::Ed25519Sha512::verify(payload, self.payload(), pk)
            }
            crate::PublicKeyInner::Secp256k1(pk) => {
                secp256k1::EcdsaSecp256k1Sha256::verify(payload, self.payload(), pk)
            }
            crate::PublicKeyInner::BlsSmall(pk) => {
                bls::BlsSmall::verify(payload, self.payload(), pk)
            }
            crate::PublicKeyInner::BlsNormal(pk) => {
                bls::BlsNormal::verify(payload, self.payload(), pk)
            }
        }?;

        Ok(())
    }
}

// TODO: Enable in ffi_import
#[cfg(not(feature = "ffi_import"))]
impl From<Signature> for (PublicKey, Vec<u8>) {
    fn from(
        Signature {
            public_key,
            payload: signature,
        }: Signature,
    ) -> Self {
        (public_key, signature.into_vec())
    }
}

// TODO: Enable in ffi_import
#[cfg(not(feature = "ffi_import"))]
impl<T> From<SignatureOf<T>> for Signature {
    fn from(SignatureOf(signature, ..): SignatureOf<T>) -> Self {
        signature
    }
}

ffi::ffi_item! {
    /// Represents signature of the data (`Block` or `Transaction` for example).
    // Lint triggers when expanding #[codec(skip)]
    #[allow(clippy::default_trait_access, clippy::unsafe_derive_deserialize)]
    #[derive(Deref, DerefMut, TypeId)]
    #[cfg_attr(not(feature="ffi_import"), derive(Decode, Encode, Serialize, Deserialize))]
    #[cfg_attr(not(feature="ffi_import"), serde(transparent))]
    // Transmute guard
    #[repr(transparent)]
    pub struct SignatureOf<T>(
        #[deref]
        #[deref_mut]
        Signature,
        #[cfg_attr(not(feature = "ffi_import"), codec(skip))] PhantomData<T>,
    );

    // SAFETY: `SignatureOf` has no trap representation in `Signature`
    ffi_type(unsafe {robust})
}

#[cfg(not(feature = "ffi_import"))]
impl<T> core::fmt::Debug for SignatureOf<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_tuple(core::any::type_name::<Self>())
            .field(&self.0)
            .finish()
    }
}

impl<T> Clone for SignatureOf<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone(), PhantomData)
    }
}

#[allow(clippy::unconditional_recursion)] // False-positive
impl<T> PartialEq for SignatureOf<T> {
    fn eq(&self, other: &Self) -> bool {
        self.0.eq(&other.0)
    }
}
impl<T> Eq for SignatureOf<T> {}

impl<T> PartialOrd for SignatureOf<T> {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl<T> Ord for SignatureOf<T> {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.0.cmp(&other.0)
    }
}

#[cfg(not(feature = "ffi_import"))]
impl<T> core::hash::Hash for SignatureOf<T> {
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

#[cfg(not(feature = "ffi_import"))]
impl<T: IntoSchema> IntoSchema for SignatureOf<T> {
    fn type_name() -> String {
        format!("SignatureOf<{}>", T::type_name())
    }
    fn update_schema_map(map: &mut iroha_schema::MetaMap) {
        if !map.contains_key::<Self>() {
            map.insert::<Self>(iroha_schema::Metadata::Tuple(
                iroha_schema::UnnamedFieldsMeta {
                    types: vec![core::any::TypeId::of::<Signature>()],
                },
            ));

            Signature::update_schema_map(map);
        }
    }
}

impl<T> SignatureOf<T> {
    /// Create [`SignatureOf`] from the given hash with [`KeyPair::private_key`].
    ///
    /// # Errors
    /// Fails if signing fails
    #[inline]
    fn from_hash(key_pair: &KeyPair, hash: HashOf<T>) -> Self {
        Self(Signature::new(key_pair, hash.as_ref()), PhantomData)
    }

    /// Verify signature for this hash
    ///
    /// # Errors
    ///
    /// Fails if the given hash didn't pass verification
    fn verify_hash(&self, hash: HashOf<T>) -> Result<(), Error> {
        self.0.verify(hash.as_ref())
    }
}

impl<T: parity_scale_codec::Encode> SignatureOf<T> {
    /// Create [`SignatureOf`] by signing the given value with [`KeyPair::private_key`].
    /// The value provided will be hashed before being signed. If you already have the
    /// hash of the value you can sign it with [`SignatureOf::from_hash`] instead.
    ///
    /// # Errors
    /// Fails if signing fails
    #[inline]
    pub fn new(key_pair: &KeyPair, value: &T) -> Self {
        Self::from_hash(key_pair, HashOf::new(value))
    }

    /// Verifies signature for this item
    ///
    /// # Errors
    /// Fails if verification fails
    pub fn verify(&self, value: &T) -> Result<(), Error> {
        self.verify_hash(HashOf::new(value))
    }
}

/// Wrapper around [`SignatureOf`] used to reimplement [`Eq`], [`Ord`], [`Hash`]
/// to compare signatures only by their [`PublicKey`].
#[derive(Deref, DerefMut, Decode, Encode, Deserialize, Serialize, IntoSchema)]
#[serde(transparent, bound(deserialize = ""))]
#[schema(transparent)]
#[repr(transparent)]
#[cfg(not(feature = "ffi_import"))]
pub struct SignatureWrapperOf<T>(
    #[deref]
    #[deref_mut]
    SignatureOf<T>,
);

#[cfg(not(feature = "ffi_import"))]
impl<T> SignatureWrapperOf<T> {
    #[inline]
    fn inner(self) -> SignatureOf<T> {
        self.0
    }
}

#[cfg(not(feature = "ffi_import"))]
impl<T> core::fmt::Debug for SignatureWrapperOf<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        self.0.fmt(f)
    }
}

#[cfg(not(feature = "ffi_import"))]
impl<T> Clone for SignatureWrapperOf<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

#[allow(clippy::unconditional_recursion)] // False-positive
#[cfg(not(feature = "ffi_import"))]
impl<T> PartialEq for SignatureWrapperOf<T> {
    fn eq(&self, other: &Self) -> bool {
        self.0.public_key().eq(other.0.public_key())
    }
}
#[cfg(not(feature = "ffi_import"))]
impl<T> Eq for SignatureWrapperOf<T> {}

#[cfg(not(feature = "ffi_import"))]
impl<T> PartialOrd for SignatureWrapperOf<T> {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
#[cfg(not(feature = "ffi_import"))]
impl<T> Ord for SignatureWrapperOf<T> {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.0.public_key().cmp(other.0.public_key())
    }
}

#[cfg(not(feature = "ffi_import"))]
impl<T> core::hash::Hash for SignatureWrapperOf<T> {
    // Implement `Hash` manually to be consistent with `Ord`
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        self.0.public_key().hash(state);
    }
}

/// Container for multiple signatures, each corresponding to a different public key.
///
/// If the public key of the added signature is already in the set,
/// the associated signature will be replaced with the new one.
///
/// GUARANTEE 1: Each signature corresponds to a different public key
#[allow(clippy::derived_hash_with_manual_eq)]
#[derive(Hash, Decode, Encode, Deserialize, Serialize, IntoSchema)]
#[serde(transparent)]
// Transmute guard
#[repr(transparent)]
#[cfg(not(feature = "ffi_import"))]
pub struct SignaturesOf<T> {
    signatures: btree_set::BTreeSet<SignatureWrapperOf<T>>,
}

#[cfg(not(feature = "ffi_import"))]
impl<T> core::fmt::Debug for SignaturesOf<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct(core::any::type_name::<Self>())
            .field("signatures", &self.signatures)
            .finish()
    }
}

#[cfg(not(feature = "ffi_import"))]
impl<T> Clone for SignaturesOf<T> {
    fn clone(&self) -> Self {
        let signatures = self.signatures.clone();
        Self { signatures }
    }
}

#[allow(clippy::unconditional_recursion)] // False-positive
#[cfg(not(feature = "ffi_import"))]
impl<T> PartialEq for SignaturesOf<T> {
    fn eq(&self, other: &Self) -> bool {
        self.signatures.eq(&other.signatures)
    }
}

#[cfg(not(feature = "ffi_import"))]
impl<T> Eq for SignaturesOf<T> {}

#[cfg(not(feature = "ffi_import"))]
impl<T> PartialOrd for SignaturesOf<T> {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

#[cfg(not(feature = "ffi_import"))]
impl<T> Ord for SignaturesOf<T> {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.signatures.cmp(&other.signatures)
    }
}

#[cfg(not(feature = "ffi_import"))]
impl<T> IntoIterator for SignaturesOf<T> {
    type Item = SignatureOf<T>;
    type IntoIter = core::iter::Map<
        btree_set::IntoIter<SignatureWrapperOf<T>>,
        fn(SignatureWrapperOf<T>) -> SignatureOf<T>,
    >;
    fn into_iter(self) -> Self::IntoIter {
        self.signatures.into_iter().map(SignatureWrapperOf::inner)
    }
}

#[cfg(not(feature = "ffi_import"))]
impl<'itm, T> IntoIterator for &'itm SignaturesOf<T> {
    type Item = &'itm SignatureOf<T>;
    type IntoIter = core::iter::Map<
        btree_set::Iter<'itm, SignatureWrapperOf<T>>,
        fn(&'itm SignatureWrapperOf<T>) -> &'itm SignatureOf<T>,
    >;
    fn into_iter(self) -> Self::IntoIter {
        self.signatures.iter().map(core::ops::Deref::deref)
    }
}

#[cfg(not(feature = "ffi_import"))]
impl<A> Extend<SignatureOf<A>> for SignaturesOf<A> {
    fn extend<T>(&mut self, iter: T)
    where
        T: IntoIterator<Item = SignatureOf<A>>,
    {
        for signature in iter {
            self.insert(signature);
        }
    }
}

#[cfg(not(feature = "ffi_import"))]
impl<T> From<SignaturesOf<T>> for btree_set::BTreeSet<SignatureOf<T>> {
    fn from(source: SignaturesOf<T>) -> Self {
        source.into_iter().collect()
    }
}

#[cfg(not(feature = "ffi_import"))]
impl<T> From<btree_set::BTreeSet<SignatureOf<T>>> for SignaturesOf<T> {
    fn from(source: btree_set::BTreeSet<SignatureOf<T>>) -> Self {
        source.into_iter().collect()
    }
}

#[cfg(not(feature = "ffi_import"))]
impl<A> From<SignatureOf<A>> for SignaturesOf<A> {
    fn from(signature: SignatureOf<A>) -> Self {
        Self {
            signatures: [SignatureWrapperOf(signature)].into(),
        }
    }
}

#[cfg(not(feature = "ffi_import"))]
impl<A> FromIterator<SignatureOf<A>> for SignaturesOf<A> {
    fn from_iter<T: IntoIterator<Item = SignatureOf<A>>>(signatures: T) -> Self {
        Self {
            signatures: signatures.into_iter().map(SignatureWrapperOf).collect(),
        }
    }
}

#[cfg(not(feature = "ffi_import"))]
impl<T> SignaturesOf<T> {
    /// Adds a signature. If the signature with this key was present, replaces it.
    pub fn insert(&mut self, signature: SignatureOf<T>) {
        self.signatures.insert(SignatureWrapperOf(signature));
    }

    /// Return all signatures.
    #[inline]
    pub fn iter(&self) -> impl ExactSizeIterator<Item = &SignatureOf<T>> {
        self.into_iter()
    }

    /// Number of signatures.
    #[inline]
    #[allow(clippy::len_without_is_empty)]
    pub fn len(&self) -> usize {
        self.signatures.len()
    }

    /// Verify signatures for this hash
    ///
    /// # Errors
    /// Fails if verificatoin of any signature fails
    pub fn verify_hash(&self, hash: HashOf<T>) -> Result<(), SignatureVerificationFail<T>> {
        self.iter().try_for_each(|signature| {
            signature
                .verify_hash(hash)
                .map_err(|error| SignatureVerificationFail {
                    signature: Box::new(signature.clone()),
                    reason: error.to_string(),
                })
        })
    }

    /// Returns true if the set is a subset of another, i.e., other contains at least all the elements in self.
    pub fn is_subset(&self, other: &Self) -> bool {
        self.signatures.is_subset(&other.signatures)
    }
}

#[cfg(not(feature = "ffi_import"))]
impl<T: Encode> SignaturesOf<T> {
    /// Create new signatures container
    ///
    /// # Errors
    /// Forwards [`SignatureOf::new`] errors
    #[inline]
    pub fn new(key_pair: &KeyPair, value: &T) -> Self {
        SignatureOf::new(key_pair, value).into()
    }

    /// Verifies all signatures
    ///
    /// # Errors
    /// Fails if validation of any signature fails
    pub fn verify(&self, item: &T) -> Result<(), SignatureVerificationFail<T>> {
        self.verify_hash(HashOf::new(item))
    }
}

/// Verification failed of some signature due to following reason
#[derive(Clone, PartialEq, Eq)]
pub struct SignatureVerificationFail<T> {
    /// Signature which verification has failed
    pub signature: Box<SignatureOf<T>>,
    /// Error which happened during verification
    pub reason: String,
}

#[cfg(not(feature = "ffi_import"))]
impl<T> core::fmt::Debug for SignatureVerificationFail<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("SignatureVerificationFail")
            .field("signature", &self.signature.0)
            .field("reason", &self.reason)
            .finish()
    }
}

#[cfg(not(feature = "ffi_import"))]
impl<T> core::fmt::Display for SignatureVerificationFail<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "Failed to verify signatures because of signature {}: {}",
            self.signature.public_key(),
            self.reason,
        )
    }
}

#[cfg(feature = "std")]
#[cfg(not(feature = "ffi_import"))]
impl<T> std::error::Error for SignatureVerificationFail<T> {}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::Algorithm;

    #[test]
    #[cfg(feature = "rand")]
    fn create_signature_ed25519() {
        let key_pair = KeyPair::random_with_algorithm(crate::Algorithm::Ed25519);
        let message = b"Test message to sign.";
        let signature = Signature::new(&key_pair, message);
        assert_eq!(*signature.public_key(), *key_pair.public_key());
        signature.verify(message).unwrap();
    }

    #[test]
    #[cfg(feature = "rand")]
    fn create_signature_secp256k1() {
        let key_pair = KeyPair::random_with_algorithm(Algorithm::Secp256k1);
        let message = b"Test message to sign.";
        let signature = Signature::new(&key_pair, message);
        assert_eq!(*signature.public_key(), *key_pair.public_key());
        signature.verify(message).unwrap();
    }

    #[test]
    #[cfg(feature = "rand")]
    fn create_signature_bls_normal() {
        let key_pair = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
        let message = b"Test message to sign.";
        let signature = Signature::new(&key_pair, message);
        assert_eq!(*signature.public_key(), *key_pair.public_key());
        signature.verify(message).unwrap();
    }

    #[test]
    #[cfg(all(feature = "rand", any(feature = "std", feature = "ffi_import")))]
    fn create_signature_bls_small() {
        let key_pair = KeyPair::random_with_algorithm(Algorithm::BlsSmall);
        let message = b"Test message to sign.";
        let signature = Signature::new(&key_pair, message);
        assert_eq!(*signature.public_key(), *key_pair.public_key());
        signature.verify(message).unwrap();
    }

    #[test]
    #[cfg(all(feature = "rand", not(feature = "ffi_import")))]
    fn signatures_of_deduplication_by_public_key() {
        let key_pair = KeyPair::random();
        let signatures = [
            SignatureOf::new(&key_pair, &1),
            SignatureOf::new(&key_pair, &2),
            SignatureOf::new(&key_pair, &3),
        ]
        .into_iter()
        .collect::<SignaturesOf<u8>>();
        // Signatures with the same public key was deduplicated
        assert_eq!(signatures.len(), 1);
    }

    #[test]
    #[cfg(not(feature = "ffi_import"))]
    fn signature_wrapper_btree_and_hash_sets_consistent_results() {
        use std::collections::{BTreeSet, HashSet};

        let keys = 5;
        let signatures_per_key = 10;
        let signatures = core::iter::repeat_with(KeyPair::random)
            .take(keys)
            .flat_map(|key| {
                core::iter::repeat_with(move || key.clone())
                    .zip(0..)
                    .map(|(key, i)| SignatureOf::new(&key, &i))
                    .take(signatures_per_key)
            })
            .map(SignatureWrapperOf)
            .collect::<Vec<_>>();
        let hash_set: HashSet<_> = signatures.clone().into_iter().collect();
        let btree_set: BTreeSet<_> = signatures.into_iter().collect();

        // Check that `hash_set` is subset of `btree_set`
        for signature in &hash_set {
            assert!(btree_set.contains(signature));
        }
        // Check that `btree_set` is subset `hash_set`
        for signature in &btree_set {
            assert!(hash_set.contains(signature));
        }
        // From the above we can conclude that `SignatureWrapperOf` have consistent behavior for `HashSet` and `BTreeSet`
    }

    #[test]
    fn signature_serialized_representation() {
        let input = json!({
            "public_key": "e701210312273E8810581E58948D3FB8F9E8AD53AAA21492EBB8703915BBB565A21B7FCC",
            "payload": "3A7991AF1ABB77F3FD27CC148404A6AE4439D095A63591B77C788D53F708A02A1509A611AD6D97B01D871E58ED00C8FD7C3917B6CA61A8C2833A19E000AAC2E4"
        });

        let signature: Signature = serde_json::from_value(input.clone()).unwrap();

        assert_eq!(serde_json::to_value(signature).unwrap(), input);
    }

    #[test]
    fn signature_from_hex_simply_reproduces_the_data() {
        let public_key = "e701210312273E8810581E58948D3FB8F9E8AD53AAA21492EBB8703915BBB565A21B7FCC";
        let payload = "3a7991af1abb77f3fd27cc148404a6ae4439d095a63591b77c788d53f708a02a1509a611ad6d97b01d871e58ed00c8fd7c3917b6ca61a8c2833a19e000aac2e4";

        let value = Signature::from_hex(public_key.parse().unwrap(), payload).unwrap();

        assert_eq!(value.public_key().to_string(), public_key);
        assert_eq!(value.payload(), hex::decode(payload).unwrap());
    }
}
