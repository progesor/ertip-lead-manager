use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("application data path error: {0}")]
    AppData(String),
    #[error("filesystem error: {0}")]
    Io(#[from] std::io::Error),
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("database migration error: {0}")]
    Migration(#[from] sqlx::migrate::MigrateError),
    #[error("unsupported import file type: {0}")]
    UnsupportedFileType(String),
    #[error("import schema error: {0}")]
    ImportSchema(String),
    #[error("CSV parse error: {0}")]
    Csv(#[from] csv::Error),
    #[error("CSV input is not valid UTF-8")]
    CsvEncoding,
    #[error("XLSX parse error: {0}")]
    Xlsx(#[from] calamine::Error),
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CommandError {
    pub code: &'static str,
    pub message: &'static str,
}

impl CommandError {
    fn app_data() -> Self {
        Self {
            code: "APP_DATA_ERROR",
            message: "Uygulama veri klasörüne erişilemedi.",
        }
    }

    fn database() -> Self {
        Self {
            code: "DATABASE_ERROR",
            message: "Yerel veritabanı işlemi tamamlanamadı.",
        }
    }

    fn import_file() -> Self {
        Self {
            code: "IMPORT_FILE_ERROR",
            message: "Seçilen lead dosyası okunamadı veya desteklenen yapıda değil.",
        }
    }
}

impl From<AppError> for CommandError {
    fn from(error: AppError) -> Self {
        log::error!("backend error: {error:?}");

        match error {
            AppError::AppData(_) | AppError::Io(_) => Self::app_data(),
            AppError::Database(_) | AppError::Migration(_) => Self::database(),
            AppError::UnsupportedFileType(_)
            | AppError::ImportSchema(_)
            | AppError::Csv(_)
            | AppError::CsvEncoding
            | AppError::Xlsx(_) => Self::import_file(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{AppError, CommandError};

    #[test]
    fn command_error_serializes_with_stable_code_and_message() {
        let value = serde_json::to_value(CommandError::database()).expect("serialize command error");

        assert_eq!(value["code"], "DATABASE_ERROR");
        assert_eq!(
            value["message"],
            "Yerel veritabanı işlemi tamamlanamadı."
        );
    }

    #[test]
    fn import_errors_have_a_stable_user_facing_category() {
        let error = CommandError::from(AppError::UnsupportedFileType("txt".to_string()));

        assert_eq!(error.code, "IMPORT_FILE_ERROR");
    }
}
