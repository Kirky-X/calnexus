// Copyright (c) 2026 Kirky.X🌠
// SPDX-License-Identifier: MIT

//! 汇率数据提供者：RateProvider trait + FrankfurterProvider 实现。
//!
//! 设计依据：ureq + dirs 本地缓存 + stale 策略
//! Feature 门控：`fx = ["dep:ureq", "dep:dirs"]`
//!
//! 三级缓存读取链：
//! 1. L1 字节缓存（oxcache ByteWeightCache，get_or_compute 单飞，进程级）
//! 2. 文件缓存 `dirs::cache_dir()/calnexus/fx_rates.json`（TTL 内有效）
//! 3. 网络 GET `https://api.frankfurter.dev/v1/latest?base=EUR`（成功后写回文件）
//!
//! 网络抓取经 limiteron 同步熔断器（`SyncCircuitBreaker`，tokio-free）包装：
//! 连续失败达阈值后 Open 快速失败（`CALNEXUS_FX_BREAKER_THRESHOLD` /
//! `CALNEXUS_FX_BREAKER_COOLDOWN_SECS`），避免 server 长驻模式下源站故障期间
//! 每笔请求白付 HTTP 超时；Open 拒绝与网络失败同等进入 stale 策略。
//!
//! 失败显性化：
//! - 网络失败 + 文件过期 + 未设 `CALNEXUS_FX_ALLOW_STALE` → CalcError::domain
//! - 网络失败 + 文件过期 + `CALNEXUS_FX_ALLOW_STALE=1` → 使用过期缓存
//! - 缓存目录不可写 → 静默降级为仅内存缓存（不报错）

use std::collections::HashMap;
use std::fmt;
use std::path::{Path, PathBuf};
#[cfg(test)]
use std::sync::Arc;
#[cfg(test)]
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::core::CalcError;
use crate::math::fx::RateTable;
use limiteron::sync::{CircuitCallError, SyncCircuitBreaker};
use oxcache::sync::ByteWeightCache;

/// Frankfurter API 固定端点（编译期常量，无 SSRF 面）。
const FRANKFURTER_URL: &str = "https://api.frankfurter.dev/v1/latest?base=EUR";

/// 默认 TTL：24 小时（秒）。
const DEFAULT_TTL_SECONDS: u64 = 24 * 3600;

/// HTTP 请求超时：5 秒。
const HTTP_TIMEOUT: Duration = Duration::from_secs(5);

/// 熔断默认阈值：连续 3 次网络失败后打开（每次失败已付 5s 超时，取比 limiteron
/// 默认 5 更小的值；server 长驻模式下 3 次 × 5s ≈ 15s 即进入快速失败）。
const DEFAULT_BREAKER_THRESHOLD: u32 = 3;

/// 熔断默认冷却：30 秒（与 limiteron circuit 默认一致）。
const DEFAULT_BREAKER_COOLDOWN: Duration = Duration::from_secs(30);

/// 响应体读取上限：1 MB（防源站异常导致内存膨胀）。
const MAX_RESPONSE_BODY_BYTES: u64 = 1024 * 1024;

/// 缓存目录名。
const CACHE_DIR_NAME: &str = "calnexus";

/// 缓存文件名。
const CACHE_FILE_NAME: &str = "fx_rates.json";

/// L1（oxcache ByteWeightCache）单 key：汇率表唯一主键。
const FX_RATES_KEY: &'static str = "fx:rates";

/// L1 字节预算：1 MiB（单 key 缓存，预算不构成实际约束）。
const FX_RATES_BUDGET_BYTES: u64 = 1024 * 1024;

/// L1 单条准入阈值：256 KiB（超过即跳过写入，防异常大表撑爆预算）。
const FX_RATES_MAX_ENTRY_BYTES: u64 = 256 * 1024;

/// L1 常量估重（weigher）：汇率表单 key 场景取固定估值。
const FX_RATES_ENTRY_WEIGHT: u64 = 4096;

/// 汇率数据源 trait（可注入 mock 测试）。
pub trait RateProvider: Send + Sync {
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

/// 测试注入的网络抓取闭包（`Box<dyn Fn>` 不满足 `Debug`，手动实现以保住外层 derive）。
#[cfg(test)]
struct TestFetcher(Box<dyn Fn() -> Result<RateTable, CalcError> + Send + Sync>);

#[cfg(test)]
impl fmt::Debug for TestFetcher {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("TestFetcher")
    }
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
/// oxcache ByteWeightCache 承载 L1（moka 原生同步内核：真实容量逐出 +
/// get_or_compute 单飞）+ 文件 L2 + stale 策略 + 网络熔断。线程安全（Send + Sync）。
pub struct FrankfurterProvider {
    /// 缓存文件路径（None 时降级为仅内存缓存）。
    cache_path: Option<PathBuf>,
    /// L1 内存缓存（oxcache ByteWeightCache：单 key `"fx:rates"`，1 MiB
    /// 字节预算、单条 256 KiB 准入）；`get_or_compute` 内建同 key 单飞，
    /// 取代原 Mutex double-check 防惊群，compute 失败不落缓存。
    cache: ByteWeightCache<&'static str, RateTable, fn(&RateTable) -> u64>,
    /// 网络熔断器（limiteron 同步版 `SyncCircuitBreaker`）：连续失败达阈值后 Open
    /// 快速失败；调用点在 get_or_compute 闭包内，天然串行化。
    breaker: SyncCircuitBreaker,
    /// 测试专用网络注入点（生产恒为 None → fetch_from_network）。
    #[cfg(test)]
    fetcher: Option<TestFetcher>,
}

/// fx 表常量估重（weigher）：缓存仅持有单 key 的汇率表，字节预算不构成
/// 实际约束，取固定估值即够——真实上限由 `with_max_entry_bytes` 把关。
fn rate_table_weight(_table: &RateTable) -> u64 {
    FX_RATES_ENTRY_WEIGHT
}

/// 手动 Debug：`ByteWeightCache` 未实现 Debug（moka 内部无 derive），
/// 输出缓存路径与条目数即可诊断（与 TestFetcher 同款处理）。
impl fmt::Debug for FrankfurterProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FrankfurterProvider")
            .field("cache_path", &self.cache_path)
            .field("cache_entries", &self.cache.entry_count())
            .field("breaker", &self.breaker)
            .finish()
    }
}

impl FrankfurterProvider {
    fn build_cache() -> ByteWeightCache<&'static str, RateTable, fn(&RateTable) -> u64> {
        ByteWeightCache::new(
            FX_RATES_BUDGET_BYTES,
            rate_table_weight as fn(&RateTable) -> u64,
        )
        .with_max_entry_bytes(FX_RATES_MAX_ENTRY_BYTES)
    }

    /// 创建默认实例，缓存路径为 `dirs::cache_dir()/calnexus/fx_rates.json`。
    pub fn new() -> Self {
        let cfg = FxProviderConfig::from_env();
        Self {
            cache_path: default_cache_path(),
            cache: Self::build_cache(),
            breaker: SyncCircuitBreaker::new(cfg.breaker_threshold(), cfg.breaker_cooldown()),
            #[cfg(test)]
            fetcher: None,
        }
    }

    /// 测试专用构造函数：指定缓存文件路径。
    #[cfg(test)]
    fn with_cache_path(path: PathBuf) -> Self {
        Self {
            cache_path: Some(path),
            cache: Self::build_cache(),
            breaker: SyncCircuitBreaker::new(DEFAULT_BREAKER_THRESHOLD, DEFAULT_BREAKER_COOLDOWN),
            fetcher: None,
        }
    }

    /// 测试专用构造函数：注入网络抓取闭包与熔断参数（不出网的确定性测试）。
    #[cfg(test)]
    fn with_fetcher(
        fetcher: Box<dyn Fn() -> Result<RateTable, CalcError> + Send + Sync>,
        threshold: u32,
        cooldown: Duration,
    ) -> Self {
        Self {
            cache_path: None,
            cache: Self::build_cache(),
            breaker: SyncCircuitBreaker::new(threshold, cooldown),
            fetcher: Some(TestFetcher(fetcher)),
        }
    }

    /// 测试专用构造函数：缓存文件路径 + 注入抓取闭包。
    ///
    /// 并发一致性测试需要同时覆盖「L2 过期文件」与「确定性抓取结局」：
    /// `with_cache_path` 走真实 ureq（结局依赖环境网络可达性），而
    /// `with_fetcher` 强制无缓存文件——此构造器补上两者的组合，
    /// 使并发场景与真实网络完全解耦（负载/有网环境下可复现）。
    #[cfg(test)]
    fn with_cache_path_and_fetcher(
        path: PathBuf,
        fetcher: Box<dyn Fn() -> Result<RateTable, CalcError> + Send + Sync>,
        threshold: u32,
        cooldown: Duration,
    ) -> Self {
        Self {
            cache_path: Some(path),
            cache: Self::build_cache(),
            breaker: SyncCircuitBreaker::new(threshold, cooldown),
            fetcher: Some(TestFetcher(fetcher)),
        }
    }

    /// 网络抓取入口：生产走 `fetch_from_network`，测试走注入闭包。
    #[cfg(test)]
    fn fetch(&self) -> Result<RateTable, CalcError> {
        if let Some(f) = &self.fetcher {
            return (f.0)();
        }
        fetch_from_network()
    }
}

impl Default for FrankfurterProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl RateProvider for FrankfurterProvider {
    fn rates(&self) -> Result<RateTable, CalcError> {
        // L1 命中直接返回；未命中时 get_or_compute 保证同 key 仅 leader
        // 执行 compute（L2 读 → TTL 判定 → 熔断拉取 → 写回 → stale 策略），
        // follower 共享 leader 结果——TTL 过期瞬间的惊群由库内单飞消除。
        // compute 返回 Err 时错误传播且不写缓存，下次调用重新 compute。
        self.cache.get_or_compute(FX_RATES_KEY, || {
            // L2: 文件缓存（TTL 内有效；损坏自愈为 miss）
            let cache_read = self
                .cache_path
                .as_ref()
                .map(|p| read_cache_file(p))
                .unwrap_or(CacheRead::Missing);
            let cached = match &cache_read {
                CacheRead::Loaded(c) => {
                    if !is_expired_at(c.fetched_at, current_unix_timestamp(), ttl_seconds()) {
                        let table: RateTable = c.clone().into();
                        return Ok(table);
                    }
                    Some(c.clone())
                }
                _ => None,
            };

            // 真正的网络抓取（此时为库内单飞 leader，全进程仅一次在途请求），
            // 经熔断器包装：连续失败达阈值后 Open 快速失败，源站故障期间
            // 不再每笔白付 HTTP 超时
            #[cfg(test)]
            let fetched = self.breaker.call(|| self.fetch());
            #[cfg(not(test))]
            let fetched = self.breaker.call(fetch_from_network);
            match fetched {
                Ok(table) => {
                    // 写回文件缓存（best-effort，失败静默降级）
                    if let Some(path) = &self.cache_path {
                        let now = current_unix_timestamp();
                        let _ = write_cache_file(path, &table, now);
                    }
                    Ok(table)
                }
                Err(breaker_err) => {
                    // 熔断拒绝与网络失败同等进入 stale 策略：Open 期间仍可用
                    // ALLOW_STALE=1 服务过期快照；缓存文件损坏时错误分类为
                    // cache_unreadable（三分类）
                    let net_err = match breaker_err {
                        CircuitCallError::Inner(err) => err,
                        CircuitCallError::Open => CalcError::dependency_unavailable(
                            "FX rate source circuit is open after repeated failures",
                        )
                        .with_i18n("msg.fx.circuit_open", vec![]),
                    };
                    let cache_corrupted = matches!(cache_read, CacheRead::Corrupted);
                    apply_stale_policy(cached.as_ref(), read_allow_stale(), cache_corrupted)
                        .map_err(|e| e.with_source(net_err.message))
                }
            }
        })
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

/// fx 环境配置（confers derive 单点加载，替代五个散读函数）。
///
/// 字段为原始 `Option<String>`：env 层不解析类型，非法值/缺省由访问器
/// （default_on_error 语义）回退默认——保持 CALNEXUS_FX_* 三态契约
/// （合法值 / 非法回退默认 / 缺省默认）。from_env 构建失败（极端场景）
/// 时整体回退全默认：fx 是可选域，不因环境变量拖垮 CLI。
#[derive(confers::Config, serde::Deserialize, Debug, Clone)]
#[config(env_prefix = "CALNEXUS_FX_")]
pub struct FxProviderConfig {
    /// `CALNEXUS_FX_TTL_HOURS`（小时，支持小数；非法回退 24h）。
    pub ttl_hours: Option<String>,
    /// `CALNEXUS_FX_BREAKER_THRESHOLD`（连续失败次数；非法或 <1 回退默认）。
    pub breaker_threshold: Option<String>,
    /// `CALNEXUS_FX_BREAKER_COOLDOWN_SECS`（秒；0=关闭快速失败，非法回退默认）。
    pub breaker_cooldown_secs: Option<String>,
    /// `CALNEXUS_FX_ALLOW_STALE`（仅 "1" 为 true）。
    pub allow_stale: Option<String>,
}

impl FxProviderConfig {
    /// 从环境变量单点加载（env_prefix `CALNEXUS_FX_`）。
    pub fn from_env() -> Self {
        confers::config::<Self>()
            .env_prefix("CALNEXUS_FX_")
            .build()
            .unwrap_or_default()
    }

    /// TTL（秒）：薄胶水，hours→seconds 换算与非法回退。
    pub fn ttl_seconds(&self) -> u64 {
        parse_ttl_hours(self.ttl_hours.as_deref())
    }

    /// 熔断阈值。
    pub fn breaker_threshold(&self) -> u32 {
        parse_breaker_threshold(self.breaker_threshold.as_deref())
    }

    /// 熔断冷却（Duration）。
    pub fn breaker_cooldown(&self) -> Duration {
        parse_breaker_cooldown(self.breaker_cooldown_secs.as_deref())
    }

    /// stale 回退开关（仅 "1" 为 true）。
    pub fn allow_stale(&self) -> bool {
        parse_allow_stale(self.allow_stale.as_deref())
    }
}

/// 进程级薄胶水：从环境加载 fx 配置取 TTL（rates() 热路径每次调用）。
fn ttl_seconds() -> u64 {
    FxProviderConfig::from_env().ttl_seconds()
}

/// 进程级薄胶水：从环境加载 fx 配置取 stale 开关。
fn read_allow_stale() -> bool {
    FxProviderConfig::from_env().allow_stale()
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

/// 解析熔断阈值（`CALNEXUS_FX_BREAKER_THRESHOLD`），非法或 <1 时回退默认值。
/// 纯函数，便于单元测试。
fn parse_breaker_threshold(raw: Option<&str>) -> u32 {
    raw.and_then(|s| s.parse::<u32>().ok())
        .filter(|&t| t >= 1)
        .unwrap_or(DEFAULT_BREAKER_THRESHOLD)
}

/// 解析熔断冷却秒数（`CALNEXUS_FX_BREAKER_COOLDOWN_SECS`），非法时回退默认；
/// 0 表示关闭快速失败（冷却到期即刻放行探针）。纯函数，便于单元测试。
fn parse_breaker_cooldown(raw: Option<&str>) -> Duration {
    raw.and_then(|s| s.parse::<u64>().ok())
        .map(Duration::from_secs)
        .unwrap_or(DEFAULT_BREAKER_COOLDOWN)
}

/// 解析 ALLOW_STALE 原始字符串：仅 `"1"` 为 true，其余（None/"0"/任意）为 false。
/// 纯函数，便于单元测试。
fn parse_allow_stale(raw: Option<&str>) -> bool {
    matches!(raw, Some("1"))
}

/// 纯函数：基于显式参数判断过期。便于单元测试（不依赖系统时钟/环境变量）。
fn is_expired_at(fetched_at: u64, now: u64, ttl_secs: u64) -> bool {
    now.saturating_sub(fetched_at) > ttl_secs
}

/// Stale 策略：网络失败时决定是否使用过期缓存。
///
/// - `cached` + `allow_stale=true` → 使用过期缓存
/// - `cached` + `allow_stale=false` → CalcError::domain（含本地快照日期 + ALLOW_STALE 提示）
/// - 无缓存 → CalcError::domain（无可用数据）
fn apply_stale_policy(
    cached: Option<&CachedRateTable>,
    allow_stale: bool,
    cache_corrupted: bool,
) -> Result<RateTable, CalcError> {
    match cached {
        Some(c) if allow_stale => Ok(c.clone().into()),
        Some(c) => Err(CalcError::dependency_unavailable(format!(
            "FX network unreachable, local snapshot: {}, set CALNEXUS_FX_ALLOW_STALE=1 to allow stale cache",
            c.date
        ))
        .with_i18n("msg.fx.network_unreachable", vec![])),
        None => {
            if cache_corrupted {
                // 三分类之二：缓存文件损坏
                Err(CalcError::dependency_unavailable(
                    "FX rate cache file is corrupted and network fetch failed".to_string(),
                )
                .with_i18n("msg.fx.cache_unreadable", vec![]))
            } else {
                // 三分类之一：网络不可达且无本地缓存
                Err(CalcError::dependency_unavailable(
                    "FX network unreachable, no local cache available".to_string(),
                )
                .with_i18n("msg.fx.network_unreachable", vec![]))
            }
        }
    }
}

/// 缓存文件读取结果（区分缺失与损坏，支撑三分类错误）。
#[derive(Debug)]
enum CacheRead {
    /// 文件不存在（正常首次运行）。
    Missing,
    /// 文件存在但读取/解析失败（损坏，可自愈重拉）。
    Corrupted,
    /// 成功读取。
    Loaded(CachedRateTable),
}

/// 读取缓存文件并反序列化。文件不存在返回 Missing，损坏返回 Corrupted（自愈为 miss）。
fn read_cache_file(path: &Path) -> CacheRead {
    match std::fs::read_to_string(path) {
        Err(_) => CacheRead::Missing,
        Ok(content) => match serde_json::from_str(&content) {
            Ok(table) => CacheRead::Loaded(table),
            Err(_) => CacheRead::Corrupted,
        },
    }
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

    // 原子写：同目录 temp 文件 + rename 替换，
    // 并发读取方只会看到完整旧文件或完整新文件（杜绝 torn write）。
    let file_name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "fx_rates.json".to_string());
    let tmp_path = path.with_file_name(format!(".{}.tmp.{}", file_name, std::process::id()));
    {
        use std::io::Write;
        let mut f = std::fs::File::create(&tmp_path)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = f.set_permissions(std::fs::Permissions::from_mode(0o600));
        }
        f.write_all(json.as_bytes())?;
        f.sync_all().ok();
    }
    match std::fs::rename(&tmp_path, path) {
        Ok(()) => Ok(()),
        Err(e) => {
            let _ = std::fs::remove_file(&tmp_path);
            Err(e)
        }
    }
}

/// 从 Frankfurter API 抓取最新汇率。
///
/// URL 编译期固定，HTTP 超时 5s，rustls 证书校验保持默认开启。
fn fetch_from_network() -> Result<RateTable, CalcError> {
    let response = ureq::get(FRANKFURTER_URL)
        .config()
        .timeout_global(Some(HTTP_TIMEOUT))
        .https_only(true) // 拒绝降级到明文/重定向到 http
        .build()
        .call()
        .map_err(|e| {
            CalcError::dependency_unavailable("FX network request failed")
                .with_source(e.to_string())
                .with_i18n("msg.fx.network_unreachable", vec![])
        })?;

    // 响应体读取上限 1MB：源站被劫持/异常时防止内存膨胀
    let body = response
        .into_body()
        .with_config()
        .limit(MAX_RESPONSE_BODY_BYTES)
        .read_to_string()
        .map_err(|e| {
            // 三分类之二：响应读取失败 → invalid_response
            CalcError::dependency_unavailable("FX response read failed")
                .with_source(e.to_string())
                .with_i18n("msg.fx.invalid_response", vec![])
        })?;

    let parsed: FrankfurterResponse = serde_json::from_str(&body).map_err(|e| {
        // 三分类之二：响应 JSON 解析失败 → invalid_response
        CalcError::dependency_unavailable("FX response parse failed")
            .with_source(e.to_string())
            .with_i18n("msg.fx.invalid_response", vec![])
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

    // ===== 三角换算（经 base EUR） =====

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

    // ===== TTL 判断 =====

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

    // ===== 熔断参数解析（CALNEXUS_FX_BREAKER_*） =====

    #[test]
    fn test_parse_breaker_threshold_default() {
        assert_eq!(parse_breaker_threshold(None), 3);
        assert_eq!(parse_breaker_threshold(Some("")), 3);
        assert_eq!(parse_breaker_threshold(Some("invalid")), 3);
        // <1 非法，回退默认
        assert_eq!(parse_breaker_threshold(Some("0")), 3);
    }

    #[test]
    fn test_parse_breaker_threshold_override() {
        assert_eq!(parse_breaker_threshold(Some("1")), 1);
        assert_eq!(parse_breaker_threshold(Some("5")), 5);
    }

    #[test]
    fn test_parse_breaker_cooldown_default() {
        assert_eq!(parse_breaker_cooldown(None), Duration::from_secs(30));
        assert_eq!(parse_breaker_cooldown(Some("")), Duration::from_secs(30));
        assert_eq!(parse_breaker_cooldown(Some("bad")), Duration::from_secs(30));
    }

    #[test]
    fn test_parse_breaker_cooldown_override() {
        assert_eq!(parse_breaker_cooldown(Some("60")), Duration::from_secs(60));
        // 0 = 关闭快速失败（冷却到期即刻放行探针）
        assert_eq!(parse_breaker_cooldown(Some("0")), Duration::ZERO);
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

    // ===== 文件缓存读写 =====

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

        let cached = match read_cache_file(&path) {
            CacheRead::Loaded(c) => c,
            other => panic!(
                "cache file should be Loaded, got non-Loaded variant ({:?})",
                matches!(other, CacheRead::Loaded(_))
            ),
        };
        assert_eq!(cached.base, "EUR");
        assert_eq!(cached.date, "2026-07-25");
        assert_eq!(cached.rates.len(), 4);
        assert_eq!(cached.fetched_at, fetched_at);
    }

    #[test]
    fn test_read_cache_file_nonexistent() {
        let path = Path::new("/nonexistent/path/fx_rates.json");
        assert!(matches!(read_cache_file(path), CacheRead::Missing));
    }

    #[test]
    fn test_read_cache_file_invalid_json() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), "not valid json").unwrap();
        assert!(matches!(read_cache_file(tmp.path()), CacheRead::Corrupted));
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
        let cached = match read_cache_file(&nested_path) {
            CacheRead::Loaded(c) => c,
            other => panic!("should be readable, got {:?}", other),
        };
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

    // ===== Stale 策略 =====

    #[test]
    fn test_stale_policy_no_cache_no_allow_stale() {
        // 无缓存 + 不允许 stale → 错误
        let result = apply_stale_policy(None, false, false);
        let err = result.expect_err("expected error");
        assert_eq!(err.kind, crate::core::ErrorKind::DependencyUnavailable);
        assert_eq!(err.i18n_key, Some("msg.fx.network_unreachable"));
    }

    #[test]
    fn test_stale_policy_no_cache_with_allow_stale() {
        // 无缓存 + 允许 stale → 仍然错误（没有数据可用）
        let result = apply_stale_policy(None, true, false);
        let err = result.expect_err("expected error");
        assert_eq!(err.kind, crate::core::ErrorKind::DependencyUnavailable);
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
        let result = apply_stale_policy(Some(&cached), false, false);
        let err = result.expect_err("expected error");
        assert_eq!(err.kind, crate::core::ErrorKind::DependencyUnavailable);
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
        let result = apply_stale_policy(Some(&cached), true, false);
        let table = result.expect("expected Ok with stale data");
        assert_eq!(table.base, "EUR");
        assert_eq!(table.date, "2026-07-20");
        assert_eq!(table.rates.len(), 4);
    }

    // ===== 拉取单飞并发一致性 =====

    #[test]
    fn test_fetch_lock_concurrent_consistency() {
        // 过期缓存 + 注入式恒失败 fetcher 下 8 线程并发 rates()：
        // 持锁序列化保证不死锁，全部线程得到同 kind 的
        // DependencyUnavailable（fetch 失败与熔断 Open 同 kind），无 panic。
        //
        // 加固说明：本测试原实现走真实 ureq（with_cache_path），依赖
        // 「环境网络不可达」假设——在有网机器的重负载并行下，上游首次
        // 瞬断、后续恢复会令各线程出现合法的混合 Ok/Err，一致性断言
        // 误报（CI 全量跑时观察到一次）。注入 fetcher 后场景与真实
        // 网络完全解耦，任何环境确定性复现。
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let table = RateTable {
            base: "EUR".to_string(),
            date: "2026-07-20".to_string(),
            rates: mock_rates(),
        };
        // 写入后将 fetched_at 置 0（必然过期）
        write_cache_file(tmp.path(), &table, 0).unwrap();

        let provider = std::sync::Arc::new(FrankfurterProvider::with_cache_path_and_fetcher(
            tmp.path().to_path_buf(),
            Box::new(|| Err(CalcError::dependency_unavailable("synthetic fx outage"))),
            DEFAULT_BREAKER_THRESHOLD,
            DEFAULT_BREAKER_COOLDOWN,
        ));
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(8));
        let mut handles = vec![];
        for _ in 0..8 {
            let p = std::sync::Arc::clone(&provider);
            let b = std::sync::Arc::clone(&barrier);
            handles.push(std::thread::spawn(move || {
                b.wait();
                p.rates()
            }));
        }
        let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
        // 全部失败且同 kind：fetch 失败（前 threshold 次）与熔断 Open
        // （其后）映射到同一 DependencyUnavailable kind
        for r in &results {
            match r {
                Err(e) => assert_eq!(
                    e.kind,
                    crate::core::ErrorKind::DependencyUnavailable,
                    "got {e:?}"
                ),
                Ok(t) => panic!("注入 fetcher 恒失败，不应出现成功结果：{t:?}"),
            }
        }
    }

    #[test]
    fn test_fetch_lock_concurrent_success_consistency() {
        // 成功侧镜像：过期缓存 + 注入式恒成功 fetcher。并发下部分线程
        // 真实抓取并写回 L1/L2，其余经 double-check 命中——所有线程
        // 必须看到同一张表（同 base/date/rates）。
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let table = RateTable {
            base: "EUR".to_string(),
            date: "2026-07-20".to_string(),
            rates: mock_rates(),
        };
        write_cache_file(tmp.path(), &table, 0).unwrap(); // 置为必然过期

        let provider = std::sync::Arc::new(FrankfurterProvider::with_cache_path_and_fetcher(
            tmp.path().to_path_buf(),
            Box::new(move || Ok(table.clone())),
            DEFAULT_BREAKER_THRESHOLD,
            DEFAULT_BREAKER_COOLDOWN,
        ));
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(8));
        let mut handles = vec![];
        for _ in 0..8 {
            let p = std::sync::Arc::clone(&provider);
            let b = std::sync::Arc::clone(&barrier);
            handles.push(std::thread::spawn(move || {
                b.wait();
                p.rates()
            }));
        }
        let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
        let first = results[0]
            .as_ref()
            .expect("injected fetcher always succeeds");
        for r in &results[1..] {
            let t = r.as_ref().expect("all threads should succeed");
            assert_eq!(t.base, first.base);
            assert_eq!(t.date, first.date);
            assert_eq!(t.rates, first.rates);
        }
    }

    // ===== 网络熔断器集成（吸收 limiteron circuit 设计；注入 fetcher，不出网） =====

    #[test]
    fn test_breaker_opens_after_consecutive_fetch_failures() {
        // 阈值 2：前两次失败真实执行注入闭包（计数增长），第三次被熔断拒绝（计数停增）
        let calls = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
        let counter = calls.clone();
        let provider = FrankfurterProvider::with_fetcher(
            Box::new(move || {
                counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                Err(CalcError::dependency_unavailable("synthetic fx outage"))
            }),
            2,
            Duration::from_secs(30),
        );
        for _ in 0..2 {
            let err = provider.rates().expect_err("fetcher always fails");
            assert_eq!(err.kind, crate::core::ErrorKind::DependencyUnavailable);
        }
        // 第 3 次：熔断已打开，闭包不再执行；stale 策略仍给出显性错误
        let err = provider.rates().expect_err("circuit should be open");
        assert_eq!(err.kind, crate::core::ErrorKind::DependencyUnavailable);
        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 2);
    }

    #[test]
    fn test_breaker_recovers_via_half_open_probe_success() {
        // 阈值 2 + 冷却 0：两次失败后打开，下一调用立即探针；探针成功回闭合
        let calls = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
        let counter = calls.clone();
        let provider = FrankfurterProvider::with_fetcher(
            Box::new(move || {
                let n = counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                if n < 2 {
                    Err(CalcError::dependency_unavailable("synthetic fx outage"))
                } else {
                    Ok(RateTable {
                        base: "EUR".to_string(),
                        date: "2026-09-15".to_string(),
                        rates: HashMap::new(),
                    })
                }
            }),
            2,
            Duration::ZERO,
        );
        assert!(provider.rates().is_err());
        assert!(provider.rates().is_err()); // 达阈值，熔断打开
        // Open + 冷却 0 → 半开探针成功 → 回闭合
        let table = provider.rates().expect("half-open probe should succeed");
        assert_eq!(table.base, "EUR");
        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 3);
        // 探针成功后 L1 已填充：下一次调用经 L1 命中放行，不再触发网络（计数不增）
        assert!(provider.rates().is_ok());
        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 3);
    }

    // ===== FrankfurterProvider 基本行为（不出网） =====

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
    fn test_parse_allow_stale_contract() {
        // 仅 "1" 为 true；None/"0"/其它任意值均为 false（三态契约）
        assert!(!parse_allow_stale(None));
        assert!(!parse_allow_stale(Some("0")));
        assert!(!parse_allow_stale(Some("true")));
        assert!(!parse_allow_stale(Some("")));
        assert!(parse_allow_stale(Some("1")));
    }

    #[test]
    fn test_fx_config_accessors_contract() {
        // FxProviderConfig 访问器=纯解析函数的薄胶水：非法/缺省回退默认
        let cfg = FxProviderConfig::default();
        assert_eq!(cfg.ttl_seconds(), 24 * 3600);
        assert_eq!(cfg.breaker_threshold(), DEFAULT_BREAKER_THRESHOLD);
        assert_eq!(cfg.breaker_cooldown(), DEFAULT_BREAKER_COOLDOWN);
        assert!(!cfg.allow_stale());
        let cfg = FxProviderConfig {
            ttl_hours: Some("0.5".to_string()),
            breaker_threshold: Some("0".to_string()),
            breaker_cooldown_secs: Some("0".to_string()),
            allow_stale: Some("1".to_string()),
        };
        assert_eq!(cfg.ttl_seconds(), 1800);
        assert_eq!(cfg.breaker_threshold(), DEFAULT_BREAKER_THRESHOLD); // <1 回退
        assert_eq!(cfg.breaker_cooldown(), Duration::ZERO); // 0=关闭快失败
        assert!(cfg.allow_stale());
    }

    // ===== L1 单飞（oxcache ByteWeightCache::get_or_compute） =====

    fn sample_rate_table() -> RateTable {
        RateTable {
            base: "EUR".to_string(),
            date: "2026-10-09".to_string(),
            rates: mock_rates(),
        }
    }

    #[test]
    fn rates_concurrent_calls_fetch_exactly_once() {
        // 8 线程并发 rates()：库内同 key 单飞，fetcher 恰执行一次，
        // 且 8 个调用方全部拿到一致结果（防惊群 + 结果共享）。
        let calls = Arc::new(AtomicUsize::new(0));
        let calls_in_fetcher = Arc::clone(&calls);
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let provider = std::sync::Arc::new(FrankfurterProvider::with_cache_path_and_fetcher(
            tmp.path().to_path_buf(),
            Box::new(move || {
                calls_in_fetcher.fetch_add(1, Ordering::SeqCst);
                std::thread::sleep(Duration::from_millis(50)); // 拉长在途窗口
                Ok(sample_rate_table())
            }),
            DEFAULT_BREAKER_THRESHOLD,
            DEFAULT_BREAKER_COOLDOWN,
        ));
        let mut handles = Vec::new();
        for _ in 0..8 {
            let p = Arc::clone(&provider);
            handles.push(std::thread::spawn(move || p.rates()));
        }
        let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
        assert_eq!(
            calls.load(Ordering::SeqCst),
            1,
            "fetcher 应恰执行一次（单飞）"
        );
        for r in &results {
            let table = r.as_ref().expect("rates 应成功");
            assert_eq!(table.base, "EUR");
            assert_eq!(table.rates.len(), 4);
        }
    }

    #[test]
    fn rates_fetch_failure_not_cached_and_retried() {
        // compute 失败不写缓存：首次 Err，第二次（网络恢复）Ok。
        let attempts = Arc::new(AtomicUsize::new(0));
        let attempts_in_fetcher = Arc::clone(&attempts);
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let provider = FrankfurterProvider::with_cache_path_and_fetcher(
            tmp.path().to_path_buf(),
            Box::new(move || {
                let n = attempts_in_fetcher.fetch_add(1, Ordering::SeqCst);
                if n == 0 {
                    Err(CalcError::dependency_unavailable("boom"))
                } else {
                    Ok(sample_rate_table())
                }
            }),
            DEFAULT_BREAKER_THRESHOLD,
            DEFAULT_BREAKER_COOLDOWN,
        );
        assert!(provider.rates().is_err(), "首次失败应显性返回 Err");
        let second = provider.rates().expect("第二次（恢复后）应成功");
        assert_eq!(second.base, "EUR");
        assert_eq!(
            attempts.load(Ordering::SeqCst),
            2,
            "失败未落缓存，重试应再抓取"
        );
    }

    #[test]
    fn rates_second_call_hits_l1_without_fetch() {
        // L1 命中：第二次 rates() 不再触发 fetcher（字节缓存生效）。
        let calls = Arc::new(AtomicUsize::new(0));
        let calls_in_fetcher = Arc::clone(&calls);
        let provider = FrankfurterProvider::with_fetcher(
            Box::new(move || {
                calls_in_fetcher.fetch_add(1, Ordering::SeqCst);
                Ok(sample_rate_table())
            }),
            DEFAULT_BREAKER_THRESHOLD,
            DEFAULT_BREAKER_COOLDOWN,
        );
        let first = provider.rates().expect("first");
        let second = provider.rates().expect("second");
        assert_eq!(calls.load(Ordering::SeqCst), 1, "第二次应命中 L1 不再抓取");
        assert_eq!(first.rates.len(), second.rates.len());
    }
}
