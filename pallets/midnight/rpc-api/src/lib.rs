// This file is part of midnight-node.
// Copyright (C) Midnight Foundation
// SPDX-License-Identifier: Apache-2.0

//! RPC API definition and types for the Midnight pallet.
//!
//! This crate contains the `#[rpc(client, server)]` trait and the associated
//! request/response types. It has no dependency on the Substrate runtime
//! or executor, making it suitable for lightweight RPC clients that don't
//! need the full node stack.
//!
//! The server-side implementation lives in `pallet-midnight-rpc`.

use serde::{Deserialize, Serialize};
use sp_storage::StorageKey;
use std::fmt::{Display, Formatter};

use jsonrpsee::{
    proc_macros::rpc,
    types::error::{ErrorObject, ErrorObjectOwned, INVALID_PARAMS_CODE},
};
pub use jsonrpsee::core::RpcResult;

pub const API_VERSIONS: [u32; 1] = [2];
pub const MAX_STATE_QUERIES: usize = 100;
pub const MAX_PATH_DEPTH: usize = 16;

/// Midnight core RPC API.
#[rpc(client, server)]
pub trait MidnightApi<BlockHash> {
    /// Returns the hex-encoded state of a deployed contract.
    #[method(name = "midnight_contractState")]
    fn get_state(
        &self,
        contract_address: String,
        at: Option<BlockHash>,
    ) -> Result<String, StateRpcError>;

    /// Returns the Merkle root of the zswap state tree.
    #[method(name = "midnight_zswapStateRoot")]
    fn get_zswap_state_root(&self, at: Option<BlockHash>) -> Result<Vec<u8>, StateRpcError>;

    /// Returns the Merkle root of the overall ledger state.
    #[method(name = "midnight_ledgerStateRoot")]
    fn get_ledger_state_root(&self, at: Option<BlockHash>) -> Result<Vec<u8>, StateRpcError>;

    /// Returns the RPC API version(s) supported by this node.
    #[method(name = "midnight_apiVersions")]
    fn get_supported_api_versions(&self) -> RpcResult<Vec<u32>>;

    /// Returns the ledger implementation version string.
    #[method(name = "midnight_ledgerVersion")]
    fn get_ledger_version(&self, at: Option<BlockHash>) -> Result<String, BlockRpcError>;

    /// Queries specific fields from a deployed contract's state tree.
    #[method(name = "midnight_queryContractState")]
    fn query_contract_state(
        &self,
        contract_address: String,
        queries: Vec<RpcStateQuery>,
        at: Option<BlockHash>,
    ) -> Result<Vec<RpcStateQueryResult>, StateRpcError>;
}

// ---------------------------------------------------------------------------
// Error types
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub enum StateRpcError {
    BadContractAddress(String),
    BadAccountAddress(String),
    UnableToGetContractState,
    UnableToGetZSwapChainState,
    UnableToGetZSwapStateRoot,
    UnableToGetLedgerStateRoot,
    TooManyQueries { max: usize, got: usize },
    PathTooDeep(usize),
}

#[derive(Debug)]
pub enum BlockRpcError {
    UnableToGetBlock(String),
    BlockNotFound,
    UnableToGetLedgerState,
    UnableToDecodeTransactions(String),
    UnableToSerializeBlock(String),
    UnableToGetChainVersion,
}

#[derive(Debug, Serialize)]
pub enum EventsError {
    HexDecode { event: String, error: String },
    Decode { event: String, error: String },
    UnableToSerializeEvent { event: String, error: String },
}

impl Display for StateRpcError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BadContractAddress(addr) => write!(f, "Unable to decode contract address: {addr}"),
            Self::BadAccountAddress(addr) => write!(f, "Unable to decode account address: {addr}"),
            Self::UnableToGetContractState => write!(f, "Unable to get requested contract state"),
            Self::UnableToGetZSwapChainState => write!(f, "Unable to get requested zswap chain state"),
            Self::UnableToGetZSwapStateRoot => write!(f, "Unable to get requested zswap state root"),
            Self::UnableToGetLedgerStateRoot => write!(f, "Unable to get requested ledger state root"),
            Self::TooManyQueries { max, got } => write!(f, "Too many queries: got {got}, maximum is {max}"),
            Self::PathTooDeep(got) => write!(f, "Path too deep: {got} steps exceeds the maximum of {MAX_PATH_DEPTH}"),
        }
    }
}

impl Display for BlockRpcError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnableToGetBlock(reason) => write!(f, "Error while getting block: {reason}"),
            Self::BlockNotFound => write!(f, "Unable to get block by hash"),
            Self::UnableToGetLedgerState => write!(f, "Unable to get ledger state"),
            Self::UnableToDecodeTransactions(reason) => write!(f, "Unable to decode transactions for block: {reason}"),
            Self::UnableToSerializeBlock(reason) => write!(f, "Unable to serialize block to JSON: {reason}"),
            Self::UnableToGetChainVersion => write!(f, "Unable to read chain name"),
        }
    }
}

impl Display for EventsError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::HexDecode { event, error } => write!(f, "Unable to hex decode event: {event}, because of {error}"),
            Self::Decode { event, error } => write!(f, "Unable to decode event: {event}, because of {error}"),
            Self::UnableToSerializeEvent { event, error } => write!(f, "Unable to serialize event to json: {event}, because of {error}"),
        }
    }
}

impl std::error::Error for StateRpcError {}
impl std::error::Error for BlockRpcError {}
impl std::error::Error for EventsError {}

impl From<StateRpcError> for ErrorObjectOwned {
    fn from(value: StateRpcError) -> Self {
        ErrorObject::owned(INVALID_PARAMS_CODE, value.to_string(), None::<()>)
    }
}

impl From<BlockRpcError> for ErrorObjectOwned {
    fn from(value: BlockRpcError) -> Self {
        ErrorObject::owned(INVALID_PARAMS_CODE, value.to_string(), None::<()>)
    }
}

impl From<EventsError> for ErrorObjectOwned {
    fn from(value: EventsError) -> Self {
        ErrorObject::owned(INVALID_PARAMS_CODE, value.to_string(), None::<()>)
    }
}

// ---------------------------------------------------------------------------
// Query types
// ---------------------------------------------------------------------------

/// A query into a contract's state tree.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcStateQuery {
    pub path: Vec<StorageKey>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcStateQueryResult {
    pub query: RpcStateQuery,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}
