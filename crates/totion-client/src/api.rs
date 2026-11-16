use std::fmt;

use reqwest::blocking::{Client, RequestBuilder};
use reqwest::Method;
use serde::de::DeserializeOwned;
use serde::Deserialize;
use totion_core::{
    AuthResponse, ChallengeRequest, ChallengeResponse, CheckinRequest, CreateBlockRequest,
    CreateCommentRequest, CreateHabitRequest, CreateJournalRequest, CreatePageRequest,
    EnsureDayResponse, RegisterRequest, Snapshot, SyncResponse, UpdateBlockRequest,
    UpdateCheckinRequest, UpdateCommentRequest, UpdateHabitRequest, UpdateJournalRequest,
    UpdatePageRequest, VerifyRequest, WriteBlockResponse, WriteCheckinResponse,
    WriteCommentResponse, WriteHabitResponse, WriteJournalResponse, WritePageResponse,
};

#[derive(Debug)]
pub enum ApiError {
    Conflict {
        message: String,
        current: serde_json::Value,
    },
    Other(String),
}

impl fmt::Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ApiError::Conflict { message, .. } => write!(f, "{message}"),
            ApiError::Other(message) => write!(f, "{message}"),
        }
    }
}

impl std::error::Error for ApiError {}

#[derive(Deserialize)]
struct ErrorBody {
    #[serde(default)]
    message: String,
    #[serde(default)]
    current: Option<serde_json::Value>,
}

pub struct Api {
    client: Client,
    base: String,
    token: Option<String>,
}

impl Api {
    pub fn new(base: &str) -> Self {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(15))
            .build()
            .expect("创建 HTTP 客户端失败");
        Self {
            client,
            base: base.trim_end_matches('/').to_string(),
            token: None,
        }
    }

    pub fn set_token(&mut self, token: Option<String>) {
        self.token = token;
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.base, path)
    }

    fn authed(&self, method: Method, path: &str) -> RequestBuilder {
        let mut request = self
            .client
            .request(method, self.url(path))
            .header("x-totion-protocol", totion_core::API_VERSION.to_string());
        if let Some(token) = &self.token {
            request = request.bearer_auth(token);
        }
        request
    }

    fn send<T: DeserializeOwned>(&self, request: RequestBuilder) -> Result<T, ApiError> {
        let response = request
            .send()
            .map_err(|err| ApiError::Other(format!("网络错误: {err}")))?;
        let status = response.status();
        if status.is_success() {
            return response
                .json::<T>()
                .map_err(|err| ApiError::Other(format!("响应解析失败: {err}")));
        }
        let body: ErrorBody = response.json().unwrap_or(ErrorBody {
            message: format!("HTTP {status}"),
            current: None,
        });
        if status == reqwest::StatusCode::CONFLICT {
            Err(ApiError::Conflict {
                message: body.message,
                current: body.current.unwrap_or(serde_json::Value::Null),
            })
        } else {
            let message = if body.message.is_empty() {
                format!("HTTP {status}")
            } else {
                body.message
            };
            Err(ApiError::Other(message))
        }
    }

    pub fn register(&self, request: &RegisterRequest) -> Result<AuthResponse, ApiError> {
        self.send(
            self.client
                .post(self.url("/api/auth/register"))
                .json(request),
        )
    }

    pub fn challenge(&self, public_key: &str) -> Result<ChallengeResponse, ApiError> {
        let body = ChallengeRequest {
            public_key: public_key.to_string(),
        };
        self.send(
            self.client
                .post(self.url("/api/auth/challenge"))
                .json(&body),
        )
    }

    pub fn verify(&self, request: &VerifyRequest) -> Result<AuthResponse, ApiError> {
        self.send(self.client.post(self.url("/api/auth/verify")).json(request))
    }

    pub fn snapshot(&self) -> Result<Snapshot, ApiError> {
        self.send(self.authed(Method::GET, "/api/snapshot"))
    }

    pub fn sync(&self, since: i64) -> Result<SyncResponse, ApiError> {
        self.send(self.authed(Method::GET, &format!("/api/sync?since={since}&limit=500")))
    }

    pub fn create_page(&self, request: &CreatePageRequest) -> Result<WritePageResponse, ApiError> {
        self.send(self.authed(Method::POST, "/api/pages").json(request))
    }

    pub fn update_page(
        &self,
        id: &str,
        request: &UpdatePageRequest,
    ) -> Result<WritePageResponse, ApiError> {
        self.send(
            self.authed(Method::PATCH, &format!("/api/pages/{id}"))
                .json(request),
        )
    }

    pub fn create_block(
        &self,
        request: &CreateBlockRequest,
    ) -> Result<WriteBlockResponse, ApiError> {
        self.send(self.authed(Method::POST, "/api/blocks").json(request))
    }

    pub fn update_block(
        &self,
        id: &str,
        request: &UpdateBlockRequest,
    ) -> Result<WriteBlockResponse, ApiError> {
        self.send(
            self.authed(Method::PATCH, &format!("/api/blocks/{id}"))
                .json(request),
        )
    }

    pub fn ensure_day(&self, date: &str) -> Result<EnsureDayResponse, ApiError> {
        self.send(self.authed(Method::POST, &format!("/api/days/{date}/ensure")))
    }

    pub fn create_habit(
        &self,
        request: &CreateHabitRequest,
    ) -> Result<WriteHabitResponse, ApiError> {
        self.send(self.authed(Method::POST, "/api/habits").json(request))
    }

    pub fn update_habit(
        &self,
        id: &str,
        request: &UpdateHabitRequest,
    ) -> Result<WriteHabitResponse, ApiError> {
        self.send(
            self.authed(Method::PATCH, &format!("/api/habits/{id}"))
                .json(request),
        )
    }

    pub fn check_in(
        &self,
        habit_id: &str,
        request: &CheckinRequest,
    ) -> Result<WriteCheckinResponse, ApiError> {
        self.send(
            self.authed(Method::POST, &format!("/api/habits/{habit_id}/checkins"))
                .json(request),
        )
    }

    pub fn update_checkin(
        &self,
        id: &str,
        request: &UpdateCheckinRequest,
    ) -> Result<WriteCheckinResponse, ApiError> {
        self.send(
            self.authed(Method::PATCH, &format!("/api/checkins/{id}"))
                .json(request),
        )
    }

    pub fn create_comment(
        &self,
        request: &CreateCommentRequest,
    ) -> Result<WriteCommentResponse, ApiError> {
        self.send(self.authed(Method::POST, "/api/comments").json(request))
    }

    pub fn update_comment(
        &self,
        id: &str,
        request: &UpdateCommentRequest,
    ) -> Result<WriteCommentResponse, ApiError> {
        self.send(
            self.authed(Method::PATCH, &format!("/api/comments/{id}"))
                .json(request),
        )
    }

    pub fn create_journal(
        &self,
        request: &CreateJournalRequest,
    ) -> Result<WriteJournalResponse, ApiError> {
        self.send(self.authed(Method::POST, "/api/journals").json(request))
    }

    pub fn update_journal(
        &self,
        id: &str,
        request: &UpdateJournalRequest,
    ) -> Result<WriteJournalResponse, ApiError> {
        self.send(
            self.authed(Method::PATCH, &format!("/api/journals/{id}"))
                .json(request),
        )
    }
}
