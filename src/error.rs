use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Kind {
    Usage,
    Auth,
    Api,
    Io,
}

impl Kind {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Usage => "usage",
            Self::Auth => "auth",
            Self::Api => "api",
            Self::Io => "io",
        }
    }

    pub(crate) fn code(self) -> i32 {
        match self {
            Self::Usage => 2,
            Self::Auth => 3,
            Self::Api => 4,
            Self::Io => 5,
        }
    }
}

pub(crate) struct Fail {
    kind: Kind,
    english: &'static str,
    russian: &'static str,
}

impl Fail {
    const fn new(kind: Kind, english: &'static str, russian: &'static str) -> Self {
        Self {
            kind,
            english,
            russian,
        }
    }

    pub(crate) fn usage() -> Self {
        Self::new(Kind::Usage, "invalid arguments", "Неверные аргументы.")
    }

    pub(crate) fn missing_credentials() -> Self {
        Self::new(
            Kind::Auth,
            "missing client credentials",
            "Нет учётных данных клиента.",
        )
    }

    pub(crate) fn denied() -> Self {
        Self::new(Kind::Auth, "authorization denied", "Авторизация отклонена.")
    }

    pub(crate) fn state_mismatch() -> Self {
        Self::new(Kind::Auth, "state mismatch", "Несовпадение state.")
    }

    pub(crate) fn missing_code() -> Self {
        Self::new(
            Kind::Auth,
            "missing authorization code",
            "Нет кода авторизации.",
        )
    }

    pub(crate) fn login_timeout() -> Self {
        Self::new(
            Kind::Auth,
            "login timed out",
            "Время ожидания входа истекло.",
        )
    }

    pub(crate) fn missing_token_file() -> Self {
        Self::new(
            Kind::Auth,
            "token file is missing",
            "Файл токенов не найден.",
        )
    }

    pub(crate) fn missing_refresh_token() -> Self {
        Self::new(
            Kind::Auth,
            "refresh token is missing",
            "Нет refresh-токена.",
        )
    }

    pub(crate) fn token_rejected() -> Self {
        Self::new(
            Kind::Auth,
            "token endpoint rejected the request",
            "Сервер токенов отклонил запрос.",
        )
    }

    pub(crate) fn bad_callback() -> Self {
        Self::new(
            Kind::Auth,
            "invalid authorization callback",
            "Некорректный ответ перенаправления.",
        )
    }

    pub(crate) fn bad_redirect() -> Self {
        Self::new(
            Kind::Io,
            "invalid redirect URL",
            "Неверный URL перенаправления.",
        )
    }

    pub(crate) fn bind_failed() -> Self {
        Self::new(
            Kind::Io,
            "failed to bind the redirect URL",
            "Не удалось занять порт перенаправления.",
        )
    }

    pub(crate) fn token_file() -> Self {
        Self::new(Kind::Io, "token file error", "Ошибка файла токенов.")
    }

    pub(crate) fn no_home() -> Self {
        Self::new(
            Kind::Io,
            "home directory is not set",
            "Не задан домашний каталог.",
        )
    }

    pub(crate) fn bad_base_url() -> Self {
        Self::new(Kind::Io, "invalid base URL", "Неверный URL сервера.")
    }

    pub(crate) fn state_entropy() -> Self {
        Self::new(
            Kind::Io,
            "failed to generate state",
            "Не удалось создать state.",
        )
    }

    pub(crate) fn token_transport() -> Self {
        Self::new(
            Kind::Api,
            "token endpoint request failed",
            "Сбой запроса к серверу токенов.",
        )
    }

    pub(crate) fn token_body() -> Self {
        Self::new(
            Kind::Api,
            "token response was not valid",
            "Ответ сервера токенов не разобран.",
        )
    }

    pub(crate) fn expiry_overflow() -> Self {
        Self::new(
            Kind::Api,
            "could not compute expiry",
            "Не удалось вычислить срок токена.",
        )
    }

    pub(crate) fn not_bearer() -> Self {
        Self::new(
            Kind::Auth,
            "token type is not Bearer",
            "Тип токена не Bearer.",
        )
    }

    pub(crate) fn user_api() -> Self {
        Self::new(
            Kind::Api,
            "user request failed",
            "Не удалось получить пользователя.",
        )
    }

    pub(crate) fn kind(&self) -> Kind {
        self.kind
    }

    pub(crate) fn code(&self) -> i32 {
        self.kind.code()
    }

    pub(crate) fn english(&self) -> &'static str {
        self.english
    }

    pub(crate) fn russian(&self) -> &'static str {
        self.russian
    }
}

impl fmt::Debug for Fail {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Fail")
            .field("kind", &self.kind)
            .field("error", &self.english)
            .finish()
    }
}

impl fmt::Display for Fail {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.russian)
    }
}

impl std::error::Error for Fail {}
