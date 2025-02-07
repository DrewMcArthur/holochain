use hdi::prelude::*;

/// an example inner value that can be serialized into the contents of Entry::App()
#[derive(Deserialize, Serialize, SerializedBytes, Debug, EntryDefRegistration)]
pub enum ThisWasmEntry {
    #[entry_def(required_validations = 5)]
    AlwaysValidates,
    #[entry_def(required_validations = 5)]
    NeverValidates,
}

impl TryFrom<&Entry> for ThisWasmEntry {
    type Error = WasmError;
    fn try_from(entry: &Entry) -> Result<Self, Self::Error> {
        match entry {
            Entry::App(eb) => Ok(Self::try_from(SerializedBytes::from(eb.to_owned()))
                .map_err(|e| wasm_error!(e))?),
            _ => Err(wasm_error!(
                "failed to deserialize ThisWasmEntry"
            )),
        }
    }
}

impl TryFrom<&ThisWasmEntry> for Entry {
    type Error = WasmError;
    fn try_from(this_wasm_entry: &ThisWasmEntry) -> Result<Self, Self::Error> {
        Ok(Entry::App(
            match AppEntryBytes::try_from(
                SerializedBytes::try_from(this_wasm_entry).map_err(|e| wasm_error!(e))?,
            ) {
                Ok(app_entry_bytes) => app_entry_bytes,
                Err(entry_error) => match entry_error {
                    EntryError::SerializedBytes(serialized_bytes_error) => {
                        return Err(wasm_error!(WasmErrorInner::Serialize(
                            serialized_bytes_error
                        )))
                    }
                    EntryError::EntryTooLarge(_) => {
                        return Err(wasm_error!(WasmErrorInner::Guest(entry_error.to_string())))
                    }
                },
            },
        ))
    }
}

impl TryFrom<&ThisWasmEntry> for ScopedEntryDefIndex {
    type Error = WasmError;

    fn try_from(_: &ThisWasmEntry) -> Result<Self, Self::Error> {
        zome_info()?
            .zome_types
            .entries
            .get(ZomeTypesKey {
                zome_index: 0.into(),
                type_index: 0.into(),
            })
            .ok_or_else(|| {
                wasm_error!(WasmErrorInner::Guest(
                    "ThisWasmEntry did not map to an EntryDefIndex within this scope".to_string(),
                ))
            })
    }
}

#[hdk_extern]
pub fn entry_defs(_: ()) -> ExternResult<EntryDefsCallbackResult> {
    Ok(EntryDefsCallbackResult::from(vec![EntryDef::from(
        ThisWasmEntry::ENTRY_DEFS[0].clone(),
    )]))
}

#[no_mangle]
pub fn __num_entry_types() -> u8 {
    1
}

#[no_mangle]
pub fn __num_link_types() -> u8 {
    0
}

#[hdk_extern]
fn validate(op: Op) -> ExternResult<ValidateCallbackResult> {
    match op {
        Op::StoreEntry(StoreEntry {
            action:
                SignedHashed {
                    hashed:
                        HoloHashed {
                            content: action, ..
                        },
                    ..
                },
            entry,
        }) => match action.app_entry_def() {
            Some(AppEntryDef { entry_index, zome_index, .. }) => {
                if zome_info()?
                    .zome_types
                    .entries
                    .find_key(ScopedZomeType {
                        zome_index: *zome_index,
                        zome_type: *entry_index,
                    })
                    .is_some()
                {
                    let entry = ThisWasmEntry::try_from(&entry)?;
                    match entry {
                        ThisWasmEntry::AlwaysValidates => Ok(ValidateCallbackResult::Valid),
                        ThisWasmEntry::NeverValidates => Ok(ValidateCallbackResult::Invalid(
                            "NeverValidates never validates".to_string(),
                        )),
                    }
                } else {
                    Ok(ValidateCallbackResult::Invalid(format!(
                        "Not a ThisWasmEntry but a {:?}",
                        action.entry_type()
                    )))
                }
            }
            None => Ok(ValidateCallbackResult::Valid),
        },
        _ => Ok(ValidateCallbackResult::Valid),
    }
}
