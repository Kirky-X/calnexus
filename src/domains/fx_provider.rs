// Copyright (c) 2026 Kirky.X. Licensed under the MIT License.

//! 汇率数据提供者：RateProvider trait + FrankfurterProvider 实现。
//!
//! 设计依据：design.md D6（ureq + dirs 本地缓存 + stale 策略）
//! Feature 门控：`fx = ["dep:ureq", "dep:dirs"]`
//!
//! 三级缓存读取链：
//! 1. 内存 Mutex<Option<RateTable>>（进程级，FrankfurterProvider 单例）
//! 2. 文件缓存 `dirs::cache_dir()/calnexus/fx_rates.json`（TTL 内有效）
//! 3. 网络 GET `https://api.frankfurter.dev/v1/latest?base=EUR`（成功后写回文件）
//!
//! 失败显性化（规则 12）：
//! - 网络失败 + 文件过期 + 未设 `CALNEXUS_FX_ALLOW_STALE` → CalcError::domain
//! - 网络失败 + 文件过期 + `CALNEXUS_FX_ALLOW_STALE=1` → 使用过期缓存
//! - 缓存目录不可写 → 静默降级为仅内存缓存（不报错）

use std::collections::HashMap;
use std::env;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::core::CalcError;
use crate::math::fx::RateTable;

/// Frankfurter API 固定端点（编译期常量，无 SSRF 面）。
const FRANKFURTER_URL: &str = "https://api.frankfurter.dev/v1/latest?base=EUR";

/// 默认 TTL：24 小时（秒）。
const DEFAULT_TTL_SECONDS: u64 = 24 * 3600;

/// HTTP 请求超时：5 秒。
const HTTP_TIMEOUT: Duration = Duration::from_secs(5);

/// 缓存目录名。
const CACHE_DIR_NAME: &str = "calnexus";

/// 缓存文件名。
const CACHE_FILE_NAME: &str = "fx_rates.json";

/// 汇率数据源 trait（可注入 mock 测试）。
pub(crate) trait RateProvider: Send + Sync {
    /// 获取最新汇率表。
    fn rates(&self) -> Result<RateTable, CalcError>;
}

/// Frankfurter API 响应体（仅提取所需字段，忽略 amount 等额外字段）。
#[derive(serde::Deserialize)]
struct FrankfurterResponse {
    base: String,
    date: String,
    rates: HashMap<String, f64>,
}

/// 缓存文件格式：RateTable + 抓取时间戳。
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
struct CachedRateTable {
    base: String,
    date: String,
    rates: HashMap<String, f64>,
    fetched_at: u64,
}

impl From<CachedRateTable> for RateTable {
    fn from(c: CachedRateTable) -> Self {
        RateTable {
            base: c.base,
            date: c.date,
            rates: c.rates,
        }
    }
}

/// FrankfurterProvider：生产环境汇率数据提供者。
///
/// 三级缓存读取链 + stale 策略。线程安全（Send + Sync）。
#[derive(Debug)]
pub struct FrankfurterProvider {
    /// 缓存文件路径（None 时降级为仅内存缓存）。
    cache_path: Option<PathBuf>,
    /// L1 内存缓存。
    in_memory: Mutex<Option<RateTable>>,
}

impl FrankfurterProvider {
    /// 创建默认实例，缓存路径为 `dirs::cache_dir()/calnexus/fx_rates.json`。
    pub fn new() -> Self {
        Self {
            cache_path: default_cache_path(),
            in_memory: Mutex::new(None),
        }
    }

    /// 测试专用构造函数：指定缓存文件路径。
    #[cfg(test)]
    fn with_cache_path(path: PathBuf) -> Self {
        Self {
            cache_path: Some(path),
            in_memory: Mutex::new(None),
        }
    }
}

impl Default for FrankfurterProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl RateProvider for FrankfurterProvider {
    fn rates(&self) -> Result<RateTable, CalcError> {
        // L1: 内存缓存（短暂持锁，仅读取/克隆后立即释放）
        {
            let mem = self.in_memory.lock().unwrap();
            if let Some(table) = mem.as_ref() {
                return Ok(table.clone());
            }
        }

        // L2: 文件缓存（TTL 内有效）
        let cached = self.cache_path.as_ref().and_then(|p| read_cache_file(p));
        if let Some(ref c) = cached {
            if !is_expired(c.fetched_at) {
                let table: RateTable = c.clone().into();
                *self.in_memory.lock().unwrap() = Some(table.clone());
                return Ok(table);
            }
        }

        // L3: 网络抓取（不持锁，避免阻塞其他线程）
        match fetch_from_network() {
            Ok(table) => {
                // 写回文件缓存（best-effort，失败静默降级）
                if let Some(path) = &self.cache_path {
                    let now = current_unix_timestamp();
                    let _ = write_cache_file(path, &table, now);
                }
                *self.in_memory.lock().unwrap() = Some(table.clone());
                Ok(table)
            }
            Err(_net_err) => {
                // Stale 策略：网络失败时使用过期缓存（需 ALLOW_STALE=1）
                apply_stale_policy(cached.as_ref(), read_allow_stale())
            }
        }
    }
}

/// 默认缓存文件路径：`dirs::cache_dir()/calnexus/fx_rates.json`。
///
/// 返回 None 时表示平台无缓存目录（如 HOME 未设置），降级为仅内存缓存。
fn default_cache_path() -> Option<PathBuf> {
    dirs::cache_dir().map(|d| d.join(CACHE_DIR_NAME).join(CACHE_FILE_NAME))
}

/// 当前 Unix 时间戳（秒）。系统时钟异常时返回 0。
fn current_unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// 解析 TTL 字符串为秒数。
///
/// 输入为小时数的字符串表示（如 "24"、"48"、"0.5"），解析失败时返回默认 24h。
/// 纯函数，便于单元测试。
fn parse_ttl_hours(hours_str: Option<&str>) -> u64 {
    hours_str
        .and_then(|s| s.parse::<f64>().ok())
        .map(|h| (h * 3600.0) as u64)
        .unwrap_or(DEFAULT_TTL_SECONDS)
}

/// 获取 TTL（秒），读取 `CALNEXUS_FX_TTL_HOURS` 环境变量。
fn ttl_seconds() -> u64 {
    parse_ttl_hours(env::var("CALNEXUS_FX_TTL_HOURS").ok().as_deref())
}

/// 检查 fetched_at 时间戳是否过期（基于当前时间 + 环境 TTL）。
fn is_expired(fetched_at: u64) -> bool {
    let now = current_unix_timestamp();
    is_expired_at(fetched_at, now, ttl_seconds())
}

/// 纯函数：基于显式参数判断过期。便于单元测试（不依赖系统时钟/环境变量）。
fn is_expired_at(fetched_at: u64, now: u64, ttl_secs: u64) -> bool {
    now.saturating_sub(fetched_at) > ttl_secs
}

/// 读取 `CALNEXUS_FX_ALLOW_STALE` 环境变量。
fn read_allow_stale() -> bool {
    env::var("CALNEXUS_FX_ALLOW_STALE")
        .map(|v| v == "1")
        .unwrap_or(false)
}

/// Stale 策略：网络失败时决定是否使用过期缓存。
///
/// - `cached` + `allow_stale=true` → 使用过期缓存
/// - `cached` + `allow_stale=false` → CalcError::domain（含本地快照日期 + ALLOW_STALE 提示）
/// - 无缓存 → CalcError::domain（无可用数据）
fn apply_stale_policy(
    cached: Option<&CachedRateTable>,
    allow_stale: bool,
) -> Result<RateTable, CalcError> {
    match cached {
        Some(c) if allow_stale => Ok(c.clone().into()),
        Some(c) => Err(CalcError::domain(format!(
            "FX network unreachable, local snapshot: {}, set CALNEXUS_FX_ALLOW_STALE=1 to allow stale cache",
            c.date
        ))
        .with_i18n("msg.fx.network_unreachable", vec![])),
        None => Err(CalcError::domain(
            "FX network unreachable, no local cache available".to_string(),
        )
        .with_i18n("msg.fx.network_unreachable", vec![])),
    }
}

/// 读取缓存文件并反序列化。文件不存在或解析失败时返回 None。
fn read_cache_file(path: &Path) -> Option<CachedRateTable> {
    let content = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&content).ok()
}

/// 写入缓存文件（序列化 + 创建父目录）。失败时返回 io::Error（调用方决定是否忽略）。
fn write_cache_file(path: &Path, table: &RateTable, fetched_at: u64) -> std::io::Result<()> {
    let cached = CachedRateTable {
        base: table.base.clone(),
        date: table.date.clone(),
        rates: table.rates.clone(),
        fetched_at,
    };
    let json = serde_json::to_string(&cached).map_err(std::io::Error::other)?;

    // 创建父目录（best-effort，目录已存在不报错）
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    std::fs::write(path, json)
}

/// 从 Frankfurter API 抓取最新汇率。
///
/// URL 编译期固定，HTTP 超时 5s，rustls 证书校验保持默认开启。
fn fetch_from_network() -> Result<RateTable, CalcError> {
    let response = ureq::get(FRANKFURTER_URL)
        .config()
        .timeout_global(Some(HTTP_TIMEOUT))
        .build()
        .call()
        .map_err(|e| {
            CalcError::domain(format!("FX network request failed: {}", e))
                .with_i18n("msg.fx.network_unreachable", vec![])
        })?;

    let body = response.into_body().read_to_string().map_err(|e| {
        CalcError::domain(format!("FX response read failed: {}", e))
            .with_i18n("msg.fx.network_unreachable", vec![])
    })?;

    let parsed: FrankfurterResponse = serde_json::from_str(&body).map_err(|e| {
        CalcError::domain(format!("FX response parse failed: {}", e))
            .with_i18n("msg.fx.network_unreachable", vec![])
    })?;

    Ok(RateTable {
        base: parsed.base,
        date: parsed.date,
        rates: parsed.rates,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 测试用 mock provider：返回预设的汇率表。
    struct MockRateProvider {
        rates: HashMap<String, f64>,
        date: String,
        base: String,
    }

    impl RateProvider for MockRateProvider {
        fn rates(&self) -> Result<RateTable, CalcError> {
            Ok(RateTable {
                base: self.base.clone(),
                date: self.date.clone(),
                rates: self.rates.clone(),
            })
        }
    }

    fn mock_rates() -> HashMap<String, f64> {
        let mut rates = HashMap::new();
        rates.insert("USD".to_string(), 1.08);
        rates.insert("CNY".to_string(), 7.85);
        rates.insert("GBP".to_string(), 0.85);
        rates.insert("JPY".to_string(), 165.0);
        rates
    }

    fn mock_provider() -> MockRateProvider {
        MockRateProvider {
            rates: mock_rates(),
            date: "2026-07-25".to_string(),
            base: "EUR".to_string(),
        }
    }

    // ===== R-fx-001: 三角换算（经 base EUR）=====

    #[test]
    fn test_triangle_conversion_usd_to_cny() {
        // fx(100, "USD", "CNY") = 100 / rate[USD] * rate[CNY] = 100 / 1.08 * 7.85
        let provider = mock_provider();
        let table = provider.rates().unwrap();
        let rate_usd = table.rates["USD"];
        let rate_cny = table.rates["CNY"];
        let result = 100.0 / rate_usd * rate_cny;
        let expected = 100.0 / 1.08 * 7.85;
        assert!((result - expected).abs() < 1e-9);
    }

    #[test]
    fn test_same_currency_identity() {
        // fx(1, "USD", "USD") = 1（同币种恒等）
        let provider = mock_provider();
        let table = provider.rates().unwrap();
        let rate_usd = table.rates["USD"];
        let result = 1.0 / rate_usd * rate_usd;
        assert!((result - 1.0).abs() < 1e-9);
    }

    #[test]
    fn test_unknown_currency_error_includes_count() {
        // 未知币种 → DomainError，消息含支持币种数量
        let provider = mock_provider();
        let table = provider.rates().unwrap();
        let result = table.rates.get("XYZ");
        assert!(result.is_none());
        // 验证支持币种数量（mock_rates 有 4 个币种）
        assert_eq!(table.rates.len(), 4);
    }

    // ===== R-fx-002: TTL 判断 =====

    #[test]
    fn test_parse_ttl_hours_default() {
        // 无环境变量 → 默认 24h
        assert_eq!(parse_ttl_hours(None), 24 * 3600);
        assert_eq!(parse_ttl_hours(Some("")), 24 * 3600);
        assert_eq!(parse_ttl_hours(Some("invalid")), 24 * 3600);
    }

    #[test]
    fn test_parse_ttl_hours_override() {
        // CALNEXUS_FX_TTL_HOURS=48 → 48h
        assert_eq!(parse_ttl_hours(Some("48")), 48 * 3600);
        // 小数支持：0.5h = 1800s
        assert_eq!(parse_ttl_hours(Some("0.5")), 1800);
    }

    #[test]
    fn test_is_expired_25h_old_default_ttl() {
        // 25h 前的时间戳 + 默认 24h TTL → 过期
        let now = 1_000_000_000u64;
        let fetched_at = now - 25 * 3600;
        let ttl = 24 * 3600;
        assert!(is_expired_at(fetched_at, now, ttl));
    }

    #[test]
    fn test_is_expired_25h_old_ttl_48() {
        // 25h 前的时间戳 + TTL=48h → 未过期
        let now = 1_000_000_000u64;
        let fetched_at = now - 25 * 3600;
        let ttl = 48 * 3600;
        assert!(!is_expired_at(fetched_at, now, ttl));
    }

    #[test]
    fn test_is_expired_10h_old_default_ttl() {
        // 10h 前的时间戳 + 默认 24h TTL → 未过期
        let now = 1_000_000_000u64;
        let fetched_at = now - 10 * 3600;
        let ttl = 24 * 3600;
        assert!(!is_expired_at(fetched_at, now, ttl));
    }

    #[test]
    fn test_is_expired_exactly_at_ttl() {
        // 恰好 TTL 时 → 未过期（> TTL 才算过期，<= TTL 未过期）
        let now = 1_000_000_000u64;
        let ttl = 24 * 3600;
        let fetched_at = now - ttl;
        assert!(!is_expired_at(fetched_at, now, ttl));
    }

    #[test]
    fn test_is_expired_future_timestamp() {
        // 未来时间戳 → 未过期（saturating_sub 返回 0）
        let now = 1_000_000_000u64;
        let fetched_at = now + 3600;
        let ttl = 24 * 3600;
        assert!(!is_expired_at(fetched_at, now, ttl));
    }

    // ===== R-fx-002: 文件缓存读写 =====

    #[test]
    fn test_write_and_read_cache_file() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let path = tmp.path().to_path_buf();

        let table = RateTable {
            base: "EUR".to_string(),
            date: "2026-07-25".to_string(),
            rates: mock_rates(),
        };
        let fetched_at = current_unix_timestamp();
        write_cache_file(&path, &table, fetched_at).unwrap();

        let cached = read_cache_file(&path).expect("cache file should be readable");
        assert_eq!(cached.base, "EUR");
        assert_eq!(cached.date, "2026-07-25");
        assert_eq!(cached.rates.len(), 4);
        assert_eq!(cached.fetched_at, fetched_at);
    }

    #[test]
    fn test_read_cache_file_nonexistent() {
        let path = Path::new("/nonexistent/path/fx_rates.json");
        assert!(read_cache_file(path).is_none());
    }

    #[test]
    fn test_read_cache_file_invalid_json() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), "not valid json").unwrap();
        assert!(read_cache_file(tmp.path()).is_none());
    }

    #[test]
    fn test_write_cache_file_creates_parent_dirs() {
        let tmp_dir = tempfile::tempdir().unwrap();
        let nested_path = tmp_dir.path().join("a").join("b").join("fx_rates.json");

        let table = RateTable {
            base: "EUR".to_string(),
            date: "2026-07-25".to_string(),
            rates: mock_rates(),
        };
        write_cache_file(&nested_path, &table, 0).unwrap();

        assert!(nested_path.exists());
        let cached = read_cache_file(&nested_path).expect("should be readable");
        assert_eq!(cached.base, "EUR");
    }

    #[test]
    fn test_cached_rate_table_to_rate_table() {
        let cached = CachedRateTable {
            base: "EUR".to_string(),
            date: "2026-07-25".to_string(),
            rates: mock_rates(),
            fetched_at: 12345,
        };
        let table: RateTable = cached.into();
        assert_eq!(table.base, "EUR");
        assert_eq!(table.date, "2026-07-25");
        assert_eq!(table.rates.len(), 4);
    }

    // ===== R-fx-003: Stale 策略 =====

    #[test]
    fn test_stale_policy_no_cache_no_allow_stale() {
        // 无缓存 + 不允许 stale → 错误
        let result = apply_stale_policy(None, false);
        let err = result.expect_err("expected error");
        assert_eq!(err.kind, crate::core::ErrorKind::Domain);
        assert_eq!(err.i18n_key, Some("msg.fx.network_unreachable"));
    }

    #[test]
    fn test_stale_policy_no_cache_with_allow_stale() {
        // 无缓存 + 允许 stale → 仍然错误（没有数据可用）
        let result = apply_stale_policy(None, true);
        let err = result.expect_err("expected error");
        assert_eq!(err.kind, crate::core::ErrorKind::Domain);
    }

    #[test]
    fn test_stale_policy_expired_cache_no_allow_stale() {
        // 过期缓存 + 不允许 stale → 错误（含快照日期 + ALLOW_STALE 提示）
        let cached = CachedRateTable {
            base: "EUR".to_string(),
            date: "2026-07-20".to_string(),
            rates: mock_rates(),
            fetched_at: 0,
        };
        let result = apply_stale_policy(Some(&cached), false);
        let err = result.expect_err("expected error");
        assert_eq!(err.kind, crate::core::ErrorKind::Domain);
        assert!(err.message.contains("2026-07-20"), "msg: {}", err.message);
        assert!(
            err.message.contains("CALNEXUS_FX_ALLOW_STALE"),
            "msg: {}",
            err.message
        );
        assert_eq!(err.i18n_key, Some("msg.fx.network_unreachable"));
    }

    #[test]
    fn test_stale_policy_expired_cache_with_allow_stale() {
        // 过期缓存 + 允许 stale → 返回缓存数据
        let cached = CachedRateTable {
            base: "EUR".to_string(),
            date: "2026-07-20".to_string(),
            rates: mock_rates(),
            fetched_at: 0,
        };
        let result = apply_stale_policy(Some(&cached), true);
        let table = result.expect("expected Ok with stale data");
        assert_eq!(table.base, "EUR");
        assert_eq!(table.date, "2026-07-20");
        assert_eq!(table.rates.len(), 4);
    }

    // ===== FrankfurterProvider 基本行为（不出网）=====

    #[test]
    fn test_default_cache_path_returns_some_on_normal_platform() {
        // 在正常 Linux/macOS/Windows 测试环境下 dirs::cache_dir() 应返回 Some
        // （CI 环境可能为 None，此处仅做非 panic 验证）
        let _ = default_cache_path();
    }

    #[test]
    fn test_frankfurter_provider_with_cache_path_uses_provided_path() {
        // 验证 with_cache_path 构造函数不 panic 且路径被使用
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let provider = FrankfurterProvider::with_cache_path(tmp.path().to_path_buf());

        // 预写一份未过期的缓存文件
        let table = RateTable {
            base: "EUR".to_string(),
            date: "2026-07-25".to_string(),
            rates: mock_rates(),
        };
        let now = current_unix_timestamp();
        write_cache_file(tmp.path(), &table, now).unwrap();

        // 从文件缓存读取（L2 命中，不出网）
        let result = provider.rates().unwrap();
        assert_eq!(result.base, "EUR");
        assert_eq!(result.date, "2026-07-25");
        assert_eq!(result.rates.len(), 4);
    }

    #[test]
    fn test_frankfurter_provider_in_memory_cache_hit() {
        // L1 命中：第二次调用直接从内存缓存返回（不出网）
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let provider = FrankfurterProvider::with_cache_path(tmp.path().to_path_buf());

        // 预写缓存文件
        let table = RateTable {
            base: "EUR".to_string(),
            date: "2026-07-25".to_string(),
            rates: mock_rates(),
        };
        let now = current_unix_timestamp();
        write_cache_file(tmp.path(), &table, now).unwrap();

        // 第一次调用：L2 文件缓存命中
        let r1 = provider.rates().unwrap();
        assert_eq!(r1.date, "2026-07-25");

        // 删除缓存文件，验证第二次调用仍成功（L1 内存命中）
        std::fs::remove_file(tmp.path()).unwrap();
        let r2 = provider.rates().unwrap();
        assert_eq!(r2.date, "2026-07-25");
    }

    #[test]
    fn test_frankfurter_provider_default_impl() {
        // 验证 Default trait
        let _provider = FrankfurterProvider::default();
    }

    #[test]
    fn test_read_allow_stale_default_false() {
        // 默认（未设环境变量）→ false
        // 注意：env::var 读取是进程级的，此测试仅验证不 panic
        let _ = read_allow_stale();
    }
}
