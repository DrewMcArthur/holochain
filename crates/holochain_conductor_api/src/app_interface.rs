use crate::{signal_subscription::SignalSubscription, ExternalApiWireError};
use holo_hash::AgentPubKey;
use holochain_keystore::LairResult;
use holochain_keystore::MetaLairClient;
use holochain_types::prelude::*;
use kitsune_p2p::dependencies::kitsune_p2p_fetch::FetchQueueInfo;
use std::collections::HashMap;

/// Represents the available conductor functions to call over an app interface
/// and will result in a corresponding [`AppResponse`] message being sent back over the
/// interface connection.
///
/// # Errors
///
/// Returns an [`AppResponse::Error`] with a reason why the request failed.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, SerializedBytes)]
#[serde(rename_all = "snake_case", tag = "type", content = "data")]
pub enum AppRequest {
    /// Get info about the app identified by the given `installed_app_id` argument,
    /// including info about each cell installed by this app.
    ///
    /// Requires `installed_app_id`, because an app interface can be the interface to multiple
    /// apps at the same time.
    ///
    /// # Returns
    ///
    /// [`AppResponse::AppInfo`]
    AppInfo {
        /// The app ID for which to get information
        installed_app_id: InstalledAppId,
    },

    /// Call a zome function. See [`ZomeCall`]
    /// to understand the data that must be provided.
    ///
    /// # Returns
    ///
    /// [`AppResponse::ZomeCalled`]
    CallZome(Box<ZomeCall>),

    /// Clone a DNA (in the biological sense), thus creating a new `Cell`.
    ///
    /// Using the provided, already-registered DNA, create a new DNA with a unique
    /// ID and the specified properties, create a new cell from this cloned DNA,
    /// and add the cell to the specified app.
    ///
    /// # Returns
    ///
    /// [`AppResponse::CloneCellCreated`]
    CreateCloneCell(Box<CreateCloneCellPayload>),

    /// Disable a clone cell.
    ///
    /// Providing a [`CloneId`] or [`CellId`], disable an existing clone cell.
    /// When the clone cell exists, it is disabled and can not be called any
    /// longer. If it doesn't exist, the call is a no-op.
    ///
    /// # Returns
    ///
    /// [`AppResponse::CloneCellDisabled`] if the clone cell existed
    /// and has been disabled.
    DisableCloneCell(Box<DisableCloneCellPayload>),

    /// Enable a clone cell that was previously disabled.
    ///
    /// # Returns
    ///
    /// [`AppResponse::CloneCellEnabled`]
    EnableCloneCell(Box<EnableCloneCellPayload>),

    /// Info about networking processes
    NetworkInfo(Box<NetworkInfoRequestPayload>),

    /// Is currently unimplemented and will return
    /// an [`AppResponse::Unimplemented`].
    SignalSubscription(SignalSubscription),
}

/// Represents the possible responses to an [`AppRequest`].
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, SerializedBytes)]
#[serde(rename_all = "snake_case", tag = "type", content = "data")]
pub enum AppResponse {
    /// This request is unimplemented
    Unimplemented(AppRequest),

    /// Can occur in response to any [`AppRequest`].
    ///
    /// There has been an error during the handling of the request.
    Error(ExternalApiWireError),

    /// The succesful response to an [`AppRequest::AppInfo`].
    ///
    /// Option will be `None` if there is no installed app with the given `installed_app_id`.
    AppInfo(Option<AppInfo>),

    /// The successful response to an [`AppRequest::CallZome`].
    ///
    /// Note that [`ExternIO`] is simply a structure of [`struct@SerializedBytes`], so the client will have
    /// to decode this response back into the data provided by the zome using a [msgpack] library to utilize it.
    ///
    /// [msgpack]: https://msgpack.org/
    ZomeCalled(Box<ExternIO>),

    /// The successful response to an [`AppRequest::CreateCloneCell`].
    ///
    /// The response contains an [`InstalledCell`] with the created clone
    /// cell's [`CloneId`] and [`CellId`].
    CloneCellCreated(InstalledCell),

    /// The successful response to an [`AppRequest::DisableCloneCell`].
    ///
    /// An existing clone cell has been disabled.
    CloneCellDisabled,

    /// The successful response to an [`AppRequest::EnableCloneCell`].
    ///
    /// A previously disabled clone cell has been enabled.
    CloneCellEnabled(InstalledCell),

    /// NetworkInfo is returned
    NetworkInfo(Vec<NetworkInfo>),
}

/// The data provided over an app interface in order to make a zome call
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ZomeCall {
    /// The ID of the cell containing the zome to be called
    pub cell_id: CellId,
    /// The zome containing the function to be called
    pub zome_name: ZomeName,
    /// The name of the zome function to call
    pub fn_name: FunctionName,
    /// The serialized data to pass as an argument to the zome function call
    pub payload: ExternIO,
    /// The capability request authorization
    ///
    /// This can be `None` and still succeed in the case where the function
    /// in the zome being called has been given an `Unrestricted` status
    /// via a `CapGrant`. Otherwise it will be necessary to provide a `CapSecret` for every call.
    pub cap_secret: Option<CapSecret>,
    /// The provenance (source) of the call
    /// MUST match the signature.
    pub provenance: AgentPubKey,
    pub signature: Signature,
    pub nonce: Nonce256Bits,
    pub expires_at: Timestamp,
}

impl From<ZomeCall> for ZomeCallUnsigned {
    fn from(zome_call: ZomeCall) -> Self {
        Self {
            cell_id: zome_call.cell_id,
            zome_name: zome_call.zome_name,
            fn_name: zome_call.fn_name,
            payload: zome_call.payload,
            cap_secret: zome_call.cap_secret,
            provenance: zome_call.provenance,
            nonce: zome_call.nonce,
            expires_at: zome_call.expires_at,
        }
    }
}

impl ZomeCall {
    pub async fn try_from_unsigned_zome_call(
        keystore: &MetaLairClient,
        unsigned_zome_call: ZomeCallUnsigned,
    ) -> LairResult<Self> {
        let signature = unsigned_zome_call
            .provenance
            .sign_raw(
                keystore,
                unsigned_zome_call
                    .data_to_sign()
                    .map_err(|e| e.to_string())?,
            )
            .await?;
        Ok(Self {
            cell_id: unsigned_zome_call.cell_id,
            zome_name: unsigned_zome_call.zome_name,
            fn_name: unsigned_zome_call.fn_name,
            payload: unsigned_zome_call.payload,
            cap_secret: unsigned_zome_call.cap_secret,
            provenance: unsigned_zome_call.provenance,
            nonce: unsigned_zome_call.nonce,
            expires_at: unsigned_zome_call.expires_at,
            signature,
        })
    }

    pub async fn resign_zome_call(
        self,
        keystore: &MetaLairClient,
        agent_key: AgentPubKey,
    ) -> LairResult<Self> {
        let zome_call_unsigned = ZomeCallUnsigned {
            provenance: agent_key,
            cell_id: self.cell_id,
            zome_name: self.zome_name,
            fn_name: self.fn_name,
            cap_secret: self.cap_secret,
            payload: self.payload,
            nonce: self.nonce,
            expires_at: self.expires_at,
        };
        ZomeCall::try_from_unsigned_zome_call(keystore, zome_call_unsigned).await
    }
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum CellInfo {
    // cells provisioned at app installation as defined in the bundle
    Provisioned(Cell),

    // cells created by cloning
    Cloned(Cell),

    // potential cells with deferred installation as defined in the bundle
    // unimplemented
    Stem(StemCell),
}

impl CellInfo {
    fn new_provisioned(
        cell_id: CellId,
        dna_modifiers: DnaModifiers,
        name: String,
        enabled: bool,
    ) -> Self {
        Self::Provisioned(Cell {
            cell_id,
            clone_id: None,
            dna_modifiers,
            name,
            enabled,
        })
    }

    fn new_cloned(
        cell_id: CellId,
        clone_id: CloneId,
        dna_modifiers: DnaModifiers,
        name: String,
        enabled: bool,
    ) -> Self {
        Self::Cloned(Cell {
            cell_id,
            clone_id: Some(clone_id),
            dna_modifiers,
            name,
            enabled,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct StemCell {
    pub dna: DnaHash,
    pub name: Option<String>,
    pub dna_modifiers: DnaModifiers,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Cell {
    pub cell_id: CellId,
    pub clone_id: Option<CloneId>,
    pub dna_modifiers: DnaModifiers,
    pub name: String,
    pub enabled: bool,
}

/// Info about an installed app, returned as part of [`AppResponse::AppInfo`]
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize, SerializedBytes)]
pub struct AppInfo {
    /// The unique identifier for an installed app in this conductor
    pub installed_app_id: InstalledAppId,
    /// Info about the cells installed in this app. Lists of cells are ordered
    /// and contain first the provisioned cell, then enabled clone cells and
    /// finally disabled clone cells.
    pub cell_info: HashMap<RoleName, Vec<CellInfo>>,
    /// The app's current status, in an API-friendly format
    pub status: AppInfoStatus,
}

impl AppInfo {
    pub fn from_installed_app(
        app: &InstalledApp,
        dna_definitions: &HashMap<CellId, DnaDefHashed>,
    ) -> Self {
        let installed_app_id = app.id().clone();
        let status = app.status().clone().into();

        let mut cell_info: HashMap<RoleName, Vec<CellInfo>> = HashMap::new();
        app.roles().iter().for_each(|(role_name, role_assignment)| {
            // create a vector with info of all cells for this role
            let mut cell_info_for_role: Vec<CellInfo> = Vec::new();

            // push the base cell to the vector of cell infos
            if let Some(provisioned_cell) = role_assignment.provisioned_cell() {
                if let Some(dna_def) = dna_definitions.get(provisioned_cell) {
                    // TODO: populate `enabled` with cell state once it is implemented for a base cell
                    let cell_info = CellInfo::new_provisioned(
                        provisioned_cell.clone(),
                        dna_def.modifiers.to_owned(),
                        dna_def.name.to_owned(),
                        status == AppInfoStatus::Running,
                    );
                    cell_info_for_role.push(cell_info);
                } else {
                    tracing::error!("no DNA definition found for cell id {}", provisioned_cell);
                }
            } else {
                // no provisioned cell, thus there must be a deferred cell
                // this is not implemented as of now
                unimplemented!()
            };

            // push enabled clone cells to the vector of cell infos
            if let Some(clone_cells) = app.clone_cells_for_role_name(role_name) {
                clone_cells.iter().for_each(|(clone_id, cell_id)| {
                    if let Some(dna_def) = dna_definitions.get(cell_id) {
                        let cell_info = CellInfo::new_cloned(
                            cell_id.to_owned(),
                            clone_id.to_owned(),
                            dna_def.modifiers.to_owned(),
                            dna_def.name.to_owned(),
                            true,
                        );
                        cell_info_for_role.push(cell_info);
                    } else {
                        tracing::error!("no DNA definition found for cell id {}", cell_id);
                    }
                });
            }

            // push disabled clone cells to the vector of cell infos
            if let Some(clone_cells) = app.disabled_clone_cells_for_role_name(role_name) {
                clone_cells.iter().for_each(|(clone_id, cell_id)| {
                    if let Some(dna_def) = dna_definitions.get(cell_id) {
                        let cell_info = CellInfo::new_cloned(
                            cell_id.to_owned(),
                            clone_id.to_owned(),
                            dna_def.modifiers.to_owned(),
                            dna_def.name.to_owned(),
                            false,
                        );
                        cell_info_for_role.push(cell_info);
                    } else {
                        tracing::error!("no DNA definition found for cell id {}", cell_id);
                    }
                });
            }

            cell_info.insert(role_name.clone(), cell_info_for_role);
        });

        Self {
            installed_app_id,
            cell_info,
            status,
        }
    }
}

/// A flat, slightly more API-friendly representation of [`AppInfo`]
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize, SerializedBytes)]
#[serde(rename_all = "snake_case")]
pub enum AppInfoStatus {
    Paused { reason: PausedAppReason },
    Disabled { reason: DisabledAppReason },
    Running,
}

impl From<AppStatus> for AppInfoStatus {
    fn from(i: AppStatus) -> Self {
        match i {
            AppStatus::Running => AppInfoStatus::Running,
            AppStatus::Disabled(reason) => AppInfoStatus::Disabled { reason },
            AppStatus::Paused(reason) => AppInfoStatus::Paused { reason },
        }
    }
}

impl From<AppInfoStatus> for AppStatus {
    fn from(i: AppInfoStatus) -> Self {
        match i {
            AppInfoStatus::Running => AppStatus::Running,
            AppInfoStatus::Disabled { reason } => AppStatus::Disabled(reason),
            AppInfoStatus::Paused { reason } => AppStatus::Paused(reason),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize, SerializedBytes)]
pub struct NetworkInfo {
    pub fetch_queue_info: FetchQueueInfo,
}

#[test]
fn status_serialization() {
    use kitsune_p2p::dependencies::kitsune_p2p_types::dependencies::serde_json;

    let status: AppInfoStatus =
        AppStatus::Disabled(DisabledAppReason::Error("because".into())).into();

    assert_eq!(
        serde_json::to_string(&status).unwrap(),
        "{\"disabled\":{\"reason\":{\"error\":\"because\"}}}"
    );

    let status: AppInfoStatus = AppStatus::Paused(PausedAppReason::Error("because".into())).into();

    assert_eq!(
        serde_json::to_string(&status).unwrap(),
        "{\"paused\":{\"reason\":{\"error\":\"because\"}}}"
    );

    let status: AppInfoStatus = AppStatus::Disabled(DisabledAppReason::User).into();

    assert_eq!(
        serde_json::to_string(&status).unwrap(),
        "{\"disabled\":{\"reason\":\"user\"}}"
    );
}
