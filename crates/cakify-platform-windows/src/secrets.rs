use std::{
    ffi::c_void,
    fmt::Write as _,
    fs::{self, File, OpenOptions},
    io::Write as _,
    iter,
    mem::size_of,
    os::windows::ffi::OsStrExt,
    path::{Path, PathBuf},
    process, ptr, slice,
    sync::atomic::{AtomicU64, Ordering},
};

use cakify_core::{SecretError, SecretId, SecretInput, SecretStore, SecretValue};
use sha2::{Digest, Sha256};
use windows::{
    core::{Error as WindowsError, PCWSTR, PWSTR},
    Win32::{
        Foundation::{LocalFree, ERROR_NOT_FOUND, HLOCAL},
        Security::{
            Credentials::{
                CredDeleteW, CredFree, CredReadW, CredWriteW, CREDENTIALW,
                CRED_PERSIST_LOCAL_MACHINE, CRED_TYPE_GENERIC,
            },
            Cryptography::{
                CryptProtectData, CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
            },
        },
        Storage::FileSystem::{MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH},
    },
};
use zeroize::Zeroize;

pub const CREDENTIAL_MANAGER_MAX_SECRET_BYTES: usize = 2_560;
const MAX_DPAPI_CIPHERTEXT_BYTES: usize = 1024 * 1024;
const MAX_SECRET_BYTES: usize = 64 * 1024;
const DPAPI_FILE_MAGIC: &[u8] = b"CAKIFY-DPAPI\0\x01";
const DPAPI_DESCRIPTION: &str = "Cakify secret";
static TEMP_FILE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Copy, Debug, Default)]
pub struct CredentialManagerSecretStore;

impl CredentialManagerSecretStore {
    fn read(&self, id: &SecretId) -> Result<SecretValue, SecretError> {
        let target = wide_string(id.as_str());
        let mut credential = ptr::null_mut();
        let result = unsafe {
            CredReadW(
                PCWSTR(target.as_ptr()),
                CRED_TYPE_GENERIC,
                None,
                &mut credential,
            )
        };
        if let Err(source) = result {
            return if is_not_found(&source) {
                Err(SecretError::NotFound { id: id.clone() })
            } else {
                Err(windows_error("cred_read", source))
            };
        }

        if credential.is_null() {
            return Err(SecretError::Corrupt {
                operation: "cred_read",
            });
        }
        let credential = CredentialBuffer(credential);
        let credential_ref = unsafe { &*credential.0 };
        let blob_size = credential_ref.CredentialBlobSize as usize;
        if blob_size == 0
            || blob_size > CREDENTIAL_MANAGER_MAX_SECRET_BYTES
            || credential_ref.CredentialBlob.is_null()
        {
            return Err(SecretError::Corrupt {
                operation: "cred_read",
            });
        }

        let value = unsafe {
            slice::from_raw_parts(credential_ref.CredentialBlob.cast_const(), blob_size).to_vec()
        };
        SecretValue::from_bytes(value).map_err(|_| SecretError::Corrupt {
            operation: "cred_read",
        })
    }
}

impl SecretStore for CredentialManagerSecretStore {
    fn put(&self, id: &SecretId, value: &SecretInput) -> Result<(), SecretError> {
        let value = value.expose_secret();
        if value.len() > CREDENTIAL_MANAGER_MAX_SECRET_BYTES {
            return Err(SecretError::InvalidValue);
        }
        let blob_size = u32::try_from(value.len()).map_err(|_| SecretError::InvalidValue)?;
        let mut target = wide_string(id.as_str());
        let credential = CREDENTIALW {
            Type: CRED_TYPE_GENERIC,
            TargetName: PWSTR(target.as_mut_ptr()),
            CredentialBlobSize: blob_size,
            CredentialBlob: value.as_ptr().cast_mut(),
            Persist: CRED_PERSIST_LOCAL_MACHINE,
            ..Default::default()
        };

        unsafe { CredWriteW(&credential, 0) }.map_err(|source| windows_error("cred_write", source))
    }

    fn get(&self, id: &SecretId) -> Result<SecretValue, SecretError> {
        self.read(id)
    }

    fn delete(&self, id: &SecretId) -> Result<(), SecretError> {
        let target = wide_string(id.as_str());
        match unsafe { CredDeleteW(PCWSTR(target.as_ptr()), CRED_TYPE_GENERIC, None) } {
            Ok(()) => Ok(()),
            Err(source) if is_not_found(&source) => Ok(()),
            Err(source) => Err(windows_error("cred_delete", source)),
        }
    }

    fn contains(&self, id: &SecretId) -> Result<bool, SecretError> {
        match self.read(id) {
            Ok(_) => Ok(true),
            Err(SecretError::NotFound { .. }) => Ok(false),
            Err(error) => Err(error),
        }
    }
}

struct CredentialBuffer(*mut CREDENTIALW);

impl Drop for CredentialBuffer {
    fn drop(&mut self) {
        if self.0.is_null() {
            return;
        }
        unsafe {
            let credential = &mut *self.0;
            let blob_size = credential.CredentialBlobSize as usize;
            if !credential.CredentialBlob.is_null()
                && blob_size > 0
                && blob_size <= CREDENTIAL_MANAGER_MAX_SECRET_BYTES
            {
                slice::from_raw_parts_mut(credential.CredentialBlob, blob_size).zeroize();
            }
            CredFree(self.0.cast::<c_void>());
        }
    }
}

#[derive(Clone, Debug)]
pub struct DpapiSecretStore {
    root: PathBuf,
}

impl DpapiSecretStore {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    fn path_for(&self, id: &SecretId) -> PathBuf {
        let digest = Sha256::digest(id.as_str().as_bytes());
        let mut file_name = String::with_capacity(64 + ".dpapi".len());
        for byte in digest {
            let _ = write!(&mut file_name, "{byte:02x}");
        }
        file_name.push_str(".dpapi");
        self.root.join(file_name)
    }

    fn protect(&self, id: &SecretId, value: &SecretInput) -> Result<Vec<u8>, SecretError> {
        let input_len =
            u32::try_from(value.expose_secret().len()).map_err(|_| SecretError::InvalidValue)?;
        let mut entropy = entropy_for(id);
        let input = CRYPT_INTEGER_BLOB {
            cbData: input_len,
            pbData: value.expose_secret().as_ptr().cast_mut(),
        };
        let entropy_blob = CRYPT_INTEGER_BLOB {
            cbData: entropy.len() as u32,
            pbData: entropy.as_mut_ptr(),
        };
        let description = wide_string(DPAPI_DESCRIPTION);
        let mut output = CRYPT_INTEGER_BLOB::default();
        let result = unsafe {
            CryptProtectData(
                &input,
                PCWSTR(description.as_ptr()),
                Some(&entropy_blob),
                None,
                None,
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            )
        };
        let output = LocalBlob(output);
        result.map_err(|source| windows_error("dpapi_protect", source))?;
        output.copy_bytes(MAX_DPAPI_CIPHERTEXT_BYTES, "dpapi_protect")
    }

    fn unprotect(&self, id: &SecretId, ciphertext: &[u8]) -> Result<SecretValue, SecretError> {
        let ciphertext_len = u32::try_from(ciphertext.len()).map_err(|_| SecretError::Corrupt {
            operation: "dpapi_read",
        })?;
        let mut ciphertext = ciphertext.to_vec();
        let mut entropy = entropy_for(id);
        let input = CRYPT_INTEGER_BLOB {
            cbData: ciphertext_len,
            pbData: ciphertext.as_mut_ptr(),
        };
        let entropy_blob = CRYPT_INTEGER_BLOB {
            cbData: entropy.len() as u32,
            pbData: entropy.as_mut_ptr(),
        };
        let mut output = CRYPT_INTEGER_BLOB::default();
        let result = unsafe {
            CryptUnprotectData(
                &input,
                None,
                Some(&entropy_blob),
                None,
                None,
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            )
        };
        let output = LocalBlob(output);
        result.map_err(|source| windows_error("dpapi_unprotect", source))?;
        let plaintext = output.copy_bytes(MAX_SECRET_BYTES, "dpapi_unprotect")?;
        SecretValue::from_bytes(plaintext).map_err(|_| SecretError::Corrupt {
            operation: "dpapi_unprotect",
        })
    }

    fn read_file(&self, id: &SecretId) -> Result<Vec<u8>, SecretError> {
        let path = self.path_for(id);
        let metadata = match fs::metadata(&path) {
            Ok(metadata) => metadata,
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => {
                return Err(SecretError::NotFound { id: id.clone() });
            }
            Err(source) => return Err(io_error("dpapi_metadata", source)),
        };
        let max_file_size = DPAPI_FILE_MAGIC.len() + size_of::<u32>() + MAX_DPAPI_CIPHERTEXT_BYTES;
        if !metadata.is_file() || metadata.len() > max_file_size as u64 {
            return Err(SecretError::Corrupt {
                operation: "dpapi_read",
            });
        }
        fs::read(path).map_err(|source| io_error("dpapi_read", source))
    }
}

impl SecretStore for DpapiSecretStore {
    fn put(&self, id: &SecretId, value: &SecretInput) -> Result<(), SecretError> {
        fs::create_dir_all(&self.root).map_err(|source| io_error("dpapi_create_dir", source))?;
        let ciphertext = self.protect(id, value)?;
        let ciphertext_len = u32::try_from(ciphertext.len()).map_err(|_| SecretError::Corrupt {
            operation: "dpapi_protect",
        })?;
        let mut payload = Vec::with_capacity(DPAPI_FILE_MAGIC.len() + 4 + ciphertext.len());
        payload.extend_from_slice(DPAPI_FILE_MAGIC);
        payload.extend_from_slice(&ciphertext_len.to_le_bytes());
        payload.extend_from_slice(&ciphertext);
        atomic_write(&self.path_for(id), &payload)
    }

    fn get(&self, id: &SecretId) -> Result<SecretValue, SecretError> {
        let payload = self.read_file(id)?;
        let ciphertext = parse_dpapi_file(&payload)?;
        self.unprotect(id, ciphertext)
    }

    fn delete(&self, id: &SecretId) -> Result<(), SecretError> {
        match fs::remove_file(self.path_for(id)) {
            Ok(()) => Ok(()),
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(source) => Err(io_error("dpapi_delete", source)),
        }
    }

    fn contains(&self, id: &SecretId) -> Result<bool, SecretError> {
        match fs::metadata(self.path_for(id)) {
            Ok(metadata) if metadata.is_file() => Ok(true),
            Ok(_) => Err(SecretError::Corrupt {
                operation: "dpapi_metadata",
            }),
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(source) => Err(io_error("dpapi_metadata", source)),
        }
    }
}

struct LocalBlob(CRYPT_INTEGER_BLOB);

impl LocalBlob {
    fn copy_bytes(&self, max_size: usize, operation: &'static str) -> Result<Vec<u8>, SecretError> {
        let size = self.0.cbData as usize;
        if size == 0 || size > max_size || self.0.pbData.is_null() {
            return Err(SecretError::Corrupt { operation });
        }
        Ok(unsafe { slice::from_raw_parts(self.0.pbData.cast_const(), size).to_vec() })
    }
}

impl Drop for LocalBlob {
    fn drop(&mut self) {
        if self.0.pbData.is_null() {
            return;
        }
        unsafe {
            let size = self.0.cbData as usize;
            if size > 0 && size <= MAX_DPAPI_CIPHERTEXT_BYTES {
                slice::from_raw_parts_mut(self.0.pbData, size).zeroize();
            }
            let _ = LocalFree(Some(HLOCAL(self.0.pbData.cast::<c_void>())));
        }
    }
}

fn parse_dpapi_file(payload: &[u8]) -> Result<&[u8], SecretError> {
    let length_offset = DPAPI_FILE_MAGIC.len();
    let data_offset = length_offset + size_of::<u32>();
    if payload.len() < data_offset || &payload[..length_offset] != DPAPI_FILE_MAGIC {
        return Err(SecretError::Corrupt {
            operation: "dpapi_read",
        });
    }
    let length = u32::from_le_bytes(payload[length_offset..data_offset].try_into().map_err(
        |_| SecretError::Corrupt {
            operation: "dpapi_read",
        },
    )?) as usize;
    if length == 0 || length > MAX_DPAPI_CIPHERTEXT_BYTES || data_offset + length != payload.len() {
        return Err(SecretError::Corrupt {
            operation: "dpapi_read",
        });
    }
    Ok(&payload[data_offset..])
}

fn atomic_write(path: &Path, payload: &[u8]) -> Result<(), SecretError> {
    let parent = path.parent().ok_or(SecretError::Corrupt {
        operation: "dpapi_path",
    })?;
    let file_name = path.file_name().ok_or(SecretError::Corrupt {
        operation: "dpapi_path",
    })?;
    let sequence = TEMP_FILE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let temporary = parent.join(format!(
        ".{}.tmp-{}-{sequence}",
        file_name.to_string_lossy(),
        process::id()
    ));

    let result = (|| {
        let mut file = open_new(&temporary)?;
        file.write_all(payload)
            .map_err(|source| io_error("dpapi_write", source))?;
        file.sync_all()
            .map_err(|source| io_error("dpapi_sync", source))?;
        drop(file);
        atomic_replace(&temporary, path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

fn open_new(path: &Path) -> Result<File, SecretError> {
    OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(path)
        .map_err(|source| io_error("dpapi_create_temp", source))
}

fn atomic_replace(source: &Path, destination: &Path) -> Result<(), SecretError> {
    let source = wide_path(source);
    let destination = wide_path(destination);
    unsafe {
        MoveFileExW(
            PCWSTR(source.as_ptr()),
            PCWSTR(destination.as_ptr()),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    }
    .map_err(|source| windows_error("dpapi_replace", source))
}

fn entropy_for(id: &SecretId) -> Vec<u8> {
    let mut digest = Sha256::new();
    digest.update(b"Cakify/DPAPI/current-user/v1\0");
    digest.update(id.as_str().as_bytes());
    digest.finalize().to_vec()
}

fn wide_string(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(iter::once(0)).collect()
}

fn wide_path(path: &Path) -> Vec<u16> {
    path.as_os_str()
        .encode_wide()
        .chain(iter::once(0))
        .collect()
}

fn is_not_found(error: &WindowsError) -> bool {
    error.code().0 == hresult_from_win32(ERROR_NOT_FOUND.0)
}

const fn hresult_from_win32(code: u32) -> i32 {
    (0x8007_0000_u32 | (code & 0x0000_FFFF)) as i32
}

fn windows_error(operation: &'static str, source: WindowsError) -> SecretError {
    SecretError::Backend {
        operation,
        code: source.code().0,
    }
}

fn io_error(operation: &'static str, source: std::io::Error) -> SecretError {
    SecretError::Io {
        operation,
        code: source.raw_os_error().unwrap_or(-1),
    }
}
