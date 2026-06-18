//! Generic versioned-document endpoints. Slot CRUD, version upload (multipart
//! for `file`, JSON body for `json`), history, download, and restore.

use std::str::FromStr;

use axum::Json;
use axum::body::Bytes;
use axum::extract::{FromRequest, Multipart, Path, Query, Request, State};
use axum::http::StatusCode;
use axum::http::header::{CONTENT_DISPOSITION, CONTENT_TYPE};
use axum::response::{IntoResponse, Response};
use serde::Deserialize;
use serde_json::Value;

use crate::application::document_service::FileUpload;
use crate::domain::ids::{DocumentId, MachineId};
use crate::domain::value_objects::{DocumentCategory, StorageKind};
use crate::error::{ApiResult, AppError};
use crate::state::AppState;
use crate::web::dto::{
    CreateDocumentRequest, DocumentDetailResponse, DocumentResponse, JsonVersionRequest,
    UpdateDocumentRequest, VersionResponse,
};
use crate::web::extractors::{AuthUser, ValidatedJson};
use crate::web::pagination::Data;
use crate::web::parse_id;

#[derive(Debug, Deserialize)]
pub struct DocumentListQuery {
    pub category: Option<String>,
}

pub async fn list_for_machine(
    State(st): State<AppState>,
    user: AuthUser,
    Path(machine_id): Path<String>,
    Query(q): Query<DocumentListQuery>,
) -> ApiResult<Json<Data<DocumentResponse>>> {
    let machine_id: MachineId = parse_id(&machine_id, "machine")?;
    let category = q.category.as_deref().map(DocumentCategory::parse);
    let docs = st
        .documents
        .list_for_machine(user.organization_id, machine_id, category)
        .await?;
    let data = docs.into_iter().map(DocumentResponse::from).collect();
    Ok(Json(Data::new(data)))
}

pub async fn create_slot(
    State(st): State<AppState>,
    user: AuthUser,
    Path(machine_id): Path<String>,
    ValidatedJson(req): ValidatedJson<CreateDocumentRequest>,
) -> ApiResult<(StatusCode, Json<DocumentResponse>)> {
    let machine_id: MachineId = parse_id(&machine_id, "machine")?;
    let category = DocumentCategory::parse(&req.category);
    let storage_kind = StorageKind::from_str(&req.storage_kind).map_err(AppError::from)?;
    let doc = st
        .documents
        .create_slot(
            user.organization_id,
            user.role,
            user.user_id,
            machine_id,
            category,
            req.name,
            storage_kind,
        )
        .await?;
    Ok((StatusCode::CREATED, Json(doc.into())))
}

pub async fn get(
    State(st): State<AppState>,
    user: AuthUser,
    Path(document_id): Path<String>,
) -> ApiResult<Json<DocumentDetailResponse>> {
    let document_id: DocumentId = parse_id(&document_id, "document")?;
    let (doc, current, versions) = st.documents.get(user.organization_id, document_id).await?;
    Ok(Json(DocumentDetailResponse {
        document: doc.into(),
        current_version: current.map(VersionResponse::from),
        versions: versions.into_iter().map(VersionResponse::from).collect(),
    }))
}

pub async fn update(
    State(st): State<AppState>,
    user: AuthUser,
    Path(document_id): Path<String>,
    ValidatedJson(req): ValidatedJson<UpdateDocumentRequest>,
) -> ApiResult<Json<DocumentResponse>> {
    let document_id: DocumentId = parse_id(&document_id, "document")?;
    let category = req.category.as_deref().map(DocumentCategory::parse);
    let doc = st
        .documents
        .rename(
            user.organization_id,
            user.role,
            document_id,
            req.name,
            category,
        )
        .await?;
    Ok(Json(doc.into()))
}

pub async fn delete(
    State(st): State<AppState>,
    user: AuthUser,
    Path(document_id): Path<String>,
) -> ApiResult<StatusCode> {
    let document_id: DocumentId = parse_id(&document_id, "document")?;
    st.documents
        .delete(user.organization_id, user.role, document_id)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

/// New version. `multipart/form-data` (fields `file`, `change_note`, `metadata`)
/// for file slots; `application/json` body for json slots.
pub async fn add_version(
    State(st): State<AppState>,
    user: AuthUser,
    Path(document_id): Path<String>,
    req: Request,
) -> ApiResult<(StatusCode, Json<VersionResponse>)> {
    let document_id: DocumentId = parse_id(&document_id, "document")?;
    let content_type = req
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();

    let version = if content_type.starts_with("multipart/form-data") {
        let mut multipart = Multipart::from_request(req, &st)
            .await
            .map_err(|e| AppError::Validation(e.to_string()))?;

        let mut file: Option<FileUpload> = None;
        let mut change_note: Option<String> = None;
        let mut metadata: Option<String> = None;

        while let Some(field) = multipart
            .next_field()
            .await
            .map_err(|e| AppError::Validation(e.to_string()))?
        {
            match field.name() {
                Some("file") => {
                    let original_filename = field
                        .file_name()
                        .map(str::to_string)
                        .unwrap_or_else(|| "file".into());
                    let mime_type = field.content_type().map(str::to_string);
                    let bytes = field
                        .bytes()
                        .await
                        .map_err(|e| AppError::Validation(e.to_string()))?;
                    file = Some(FileUpload {
                        original_filename,
                        mime_type,
                        bytes: bytes.to_vec(),
                    });
                }
                Some("change_note") => {
                    change_note = Some(
                        field
                            .text()
                            .await
                            .map_err(|e| AppError::Validation(e.to_string()))?,
                    );
                }
                Some("metadata") => {
                    let raw = field
                        .text()
                        .await
                        .map_err(|e| AppError::Validation(e.to_string()))?;
                    metadata = Some(normalize_json(&raw)?);
                }
                _ => {
                    let _ = field.bytes().await;
                }
            }
        }

        let upload = file.ok_or_else(|| AppError::Validation("missing 'file' field".into()))?;
        st.documents
            .add_file_version(
                user.organization_id,
                user.role,
                user.user_id,
                document_id,
                upload,
                change_note,
                metadata,
            )
            .await?
    } else {
        let bytes = Bytes::from_request(req, &st)
            .await
            .map_err(|e| AppError::Validation(e.to_string()))?;
        let body: JsonVersionRequest =
            serde_json::from_slice(&bytes).map_err(|e| AppError::Validation(e.to_string()))?;
        let is_object_or_array = body.content.is_object() || body.content.is_array();
        let content_json = body.content.to_string();
        let metadata = body.metadata.map(|m| m.to_string());
        st.documents
            .add_json_version(
                user.organization_id,
                user.role,
                user.user_id,
                document_id,
                content_json,
                is_object_or_array,
                body.change_note,
                metadata,
            )
            .await?
    };

    Ok((StatusCode::CREATED, Json(version.into())))
}

pub async fn list_versions(
    State(st): State<AppState>,
    user: AuthUser,
    Path(document_id): Path<String>,
) -> ApiResult<Json<Data<VersionResponse>>> {
    let document_id: DocumentId = parse_id(&document_id, "document")?;
    let versions = st
        .documents
        .list_versions(user.organization_id, document_id)
        .await?;
    let data = versions.into_iter().map(VersionResponse::from).collect();
    Ok(Json(Data::new(data)))
}

pub async fn get_version(
    State(st): State<AppState>,
    user: AuthUser,
    Path((document_id, version_no)): Path<(String, i32)>,
) -> ApiResult<Json<VersionResponse>> {
    let document_id: DocumentId = parse_id(&document_id, "document")?;
    let version = st
        .documents
        .get_version(user.organization_id, document_id, version_no)
        .await?;
    Ok(Json(version.into()))
}

pub async fn download(
    State(st): State<AppState>,
    user: AuthUser,
    Path((document_id, version_no)): Path<(String, i32)>,
) -> ApiResult<Response> {
    let document_id: DocumentId = parse_id(&document_id, "document")?;
    let (version, bytes) = st
        .documents
        .download(user.organization_id, document_id, version_no)
        .await?;
    let mime = version
        .mime_type
        .unwrap_or_else(|| "application/octet-stream".to_string());
    let filename = version
        .original_filename
        .unwrap_or_else(|| format!("v{version_no}"));
    let disposition = format!("attachment; filename=\"{}\"", filename.replace('"', ""));
    let headers = [(CONTENT_TYPE, mime), (CONTENT_DISPOSITION, disposition)];
    Ok((headers, bytes).into_response())
}

pub async fn restore(
    State(st): State<AppState>,
    user: AuthUser,
    Path((document_id, version_no)): Path<(String, i32)>,
) -> ApiResult<(StatusCode, Json<VersionResponse>)> {
    let document_id: DocumentId = parse_id(&document_id, "document")?;
    let version = st
        .documents
        .restore(
            user.organization_id,
            user.role,
            user.user_id,
            document_id,
            version_no,
        )
        .await?;
    Ok((StatusCode::CREATED, Json(version.into())))
}

/// Validate that a multipart `metadata` field is JSON and normalize it.
fn normalize_json(raw: &str) -> ApiResult<String> {
    let value: Value = serde_json::from_str(raw)
        .map_err(|_| AppError::Validation("metadata must be JSON".into()))?;
    Ok(value.to_string())
}
