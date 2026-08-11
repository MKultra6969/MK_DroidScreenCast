//! Ошибки IPC-команд `api_*`.

use serde::Serialize;

/// Ошибка команды в том же виде, в каком её отдаёт FastAPI.
///
/// Роутер в `frontend/src/lib/api.ts` собирает из неё настоящий `Response` со
/// статусом `status` и телом `{"detail": "..."}`. Именно эту форму уже
/// разбирают `readJson()` и `fileErrorMessage()` в `App.tsx`, поэтому менять
/// её нельзя: иначе пришлось бы править все 40+ мест вызова `apiFetch`.
#[derive(Debug, Clone, Serialize)]
pub struct ApiError {
    /// HTTP-статус, который роутер поставит синтетическому `Response`.
    pub status: u16,
    /// Текст ошибки — попадает в поле `detail` тела ответа.
    pub detail: String,
}

impl ApiError {
    /// Ошибка с произвольным статусом. Статусы использовать те же, что
    /// отдаёт соответствующий HTTP-эндпоинт (400, 404, 409, 500).
    pub fn new(status: u16, detail: impl Into<String>) -> Self {
        Self {
            status,
            detail: detail.into(),
        }
    }

    /// 500 — сбой на стороне приложения.
    pub fn internal(detail: impl Into<String>) -> Self {
        Self::new(500, detail)
    }
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} {}", self.status, self.detail)
    }
}

impl std::error::Error for ApiError {}
