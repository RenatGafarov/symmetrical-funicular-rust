use std::env;
use std::fs;
use std::path::Path;

/// Ошибка загрузки конфигурации
#[derive(Debug)]
pub struct ConfigError {
    pub message: String,
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ConfigError: {}", self.message)
    }
}

impl std::error::Error for ConfigError {}

impl ConfigError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

/// Загружает конфигурацию из .env файла
pub fn load() -> Result<(), ConfigError> {
    load_from_path(".env")
}

/// Загружает конфигурацию из указанного .env файла
pub fn load_from_path(path: impl AsRef<Path>) -> Result<(), ConfigError> {
    let path = path.as_ref();

    if !path.exists() {
        return Err(ConfigError::new(format!(
            "Config file not found: {}",
            path.display()
        )));
    }

    let content = fs::read_to_string(path)
        .map_err(|e| ConfigError::new(format!("Failed to read {}: {}", path.display(), e)))?;

    for line in content.lines() {
        let line = line.trim();

        // Пропускаем пустые строки и комментарии
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        // Парсим KEY=VALUE
        if let Some((key, value)) = line.split_once('=') {
            let key = key.trim();
            let value = value.trim();

            // Убираем кавычки если есть
            let value = value
                .strip_prefix('"')
                .and_then(|v| v.strip_suffix('"'))
                .or_else(|| value.strip_prefix('\'').and_then(|v| v.strip_suffix('\'')))
                .unwrap_or(value);

            // Устанавливаем переменную окружения только если она ещё не установлена
            if env::var(key).is_err() {
                // SAFETY: вызывается однократно при запуске до создания потоков
                unsafe { env::set_var(key, value) };
            }
        }
    }

    Ok(())
}

/// Получает обязательное значение переменной окружения
pub fn require(key: &str) -> Result<String, ConfigError> {
    env::var(key).map_err(|_| ConfigError::new(format!("Missing required env var: {}", key)))
}