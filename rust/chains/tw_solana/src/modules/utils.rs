// SPDX-License-Identifier: Apache-2.0
//
// Copyright © 2017 Trust Wallet.

use crate::modules::tx_signer::TxSigner;
use crate::transaction::versioned::VersionedTransaction;
use crate::SOLANA_ALPHABET;
use std::borrow::Cow;
use tw_coin_entry::error::{SigningError, SigningErrorType, SigningResult};
use tw_coin_entry::signing_output_error;
use tw_encoding::{base58, base64};
use tw_hash::H256;
use tw_keypair::{ed25519, KeyPairResult};
use tw_memory::Data;
use tw_proto::Solana::Proto;

pub struct SolanaTransaction;

impl SolanaTransaction {
    pub fn update_blockhash_and_sign(
        encoded_tx: &str,
        recent_blockhash: &str,
        private_keys: &[Data],
    ) -> Proto::SigningOutput<'static> {
        Self::update_blockhash_and_sign_impl(encoded_tx, recent_blockhash, private_keys)
            .unwrap_or_else(|e| signing_output_error!(Proto::SigningOutput, e))
    }

    fn update_blockhash_and_sign_impl(
        encoded_tx: &str,
        recent_blockhash: &str,
        private_keys: &[Data],
    ) -> SigningResult<Proto::SigningOutput<'static>> {
        let is_url = false;
        let tx_bytes = base64::decode(encoded_tx, is_url)?;

        let mut tx_to_sign: VersionedTransaction = bincode::deserialize(&tx_bytes)
            .map_err(|_| SigningError(SigningErrorType::Error_input_parse))?;

        let new_blockchain_hash = base58::decode(recent_blockhash, &SOLANA_ALPHABET)?;
        let new_blockchain_hash = H256::try_from(new_blockchain_hash.as_slice())
            .map_err(|_| SigningError(SigningErrorType::Error_invalid_params))?;

        // Update the transaction's blockhash and re-sign it.
        tx_to_sign.message.set_recent_blockhash(new_blockchain_hash);

        let unsigned_encoded = TxSigner::preimage_versioned(&tx_to_sign)?;

        // Do not sign the transaction if there is no private keys, but set zeroed signatures.
        // It's needed to estimate the transaction fee with an updated blockhash without using real private keys.
        let signed_tx = if private_keys.is_empty() {
            tx_to_sign.zeroize_signatures();
            tx_to_sign
        } else {
            let private_keys = private_keys
                .iter()
                .map(|pk| ed25519::sha512::PrivateKey::try_from(pk.as_slice()))
                .collect::<KeyPairResult<Vec<_>>>()?;

            TxSigner::sign_versioned(tx_to_sign, &private_keys)?
        };

        let unsigned_encoded = base64::encode(&unsigned_encoded, is_url);
        let signed_encoded = bincode::serialize(&signed_tx)
            .map_err(|_| SigningError(SigningErrorType::Error_internal))?;
        let signed_encoded = base64::encode(&signed_encoded, is_url);
        let message_encoded = bincode::serialize(&signed_tx.message)
            .map_err(|_| SigningError(SigningErrorType::Error_internal))?;
        let message_encoded = base64::encode(&message_encoded, is_url);

        Ok(Proto::SigningOutput {
            encoded: Cow::from(signed_encoded),
            unsigned_tx: Cow::from(unsigned_encoded),
            message_encoded: Cow::from(message_encoded),
            ..Proto::SigningOutput::default()
        })
    }
}
