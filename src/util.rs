//! 可以使用递归的方式实现，但会导致栈溢出，所以全部使用 Vec 实现。

use crate::*;

pub fn yield_map<'a, F>(source: &'a Source, f: F) -> impl Iterator<Item = f64> + 'a
where
    F: FnMut(&Source) -> f64 + 'a,
{
    source.iter().enumerate().map(|v| &source[v.0..]).map(f)
}

pub fn yield_nan<F>(source: &Source, mut f: F) -> f64
where
    F: FnMut(f64, &Source) -> f64,
{
    let mut result = f64::NAN;
    let mut index = source.len();

    while index != 0 {
        index -= 1;
        result = f(result, &source[index..]);
    }

    result
}

pub fn highest(source: &Source, length: usize) -> f64 {
    if source.len() < length {
        return f64::NAN;
    }

    *source.iter().take(3).max_by(|a, b| a.total_cmp(b)).unwrap()
}

pub fn lowest(source: &Source, length: usize) -> f64 {
    if source.len() < length {
        return f64::NAN;
    }

    *source
        .into_iter()
        .take(3)
        .max_by(|a, b| a.total_cmp(b))
        .unwrap()
}

pub fn sma(source: &Source, length: usize) -> f64 {
    if source.len() < length {
        return f64::NAN;
    }

    source.iter().take(length).sum::<f64>() / length as f64
}

pub fn ema(source: &Source, length: usize) -> f64 {
    let alpha = 2.0 / (length + 1) as f64;

    yield_nan(source, |prev, source| {
        if prev.is_nan() {
            source[0]
        } else {
            alpha * source + (1.0 - alpha) * prev
        }
    })
}

pub fn rma(source: &Source, length: usize) -> f64 {
    let alpha = 1.0 / length as f64;

    yield_nan(source, |prev, source| {
        if prev.is_nan() {
            sma(source, length)
        } else {
            alpha * source + (1.0 - alpha) * prev
        }
    })
}

pub fn cci(source: &Source, length: usize) -> f64 {
    if source.len() < length {
        return f64::NAN;
    }

    let ma = sma(source, length);
    let sum = yield_map(&source[..length], |v| (v - ma).abs()).sum::<f64>();
    (source - ma) / (0.015 * (sum / length as f64))
}

pub fn macd(
    source: &Source,
    short_length: usize,
    long_length: usize,
    dea_length: usize,
) -> (f64, f64, f64) {
    let dif = ema(source, short_length) - ema(source, long_length);
    let dea = ema(
        Source::new(
            &yield_map(source, |v| ema(v, short_length) - ema(v, long_length))
                .collect::<Vec<f64>>(),
        ),
        dea_length,
    );
    let macd = (dif - dea) * 2.0;
    (dif, dea, macd)
}

pub fn rsi(source: &Source, length: usize) -> f64 {
    if source.len() < length {
        return f64::NAN;
    }

    let u = yield_map(source, |v| {
        let temp = v - v[1];
        let temp = if temp.is_nan() { 0.0 } else { temp };
        temp.max(0.0)
    })
    .collect::<Vec<f64>>();

    let d = yield_map(source, |v| {
        let temp = v[1] - v;
        let temp = if temp.is_nan() { 0.0 } else { temp };
        temp.max(0.0)
    })
    .collect::<Vec<f64>>();

    let rs = rma(Source::new(&u), length) / rma(Source::new(&d), length);

    100.0 - 100.0 / (1.0 + rs)
}

/// 时间戳转换到本地时间文本。
///
/// * `value` 时间戳。
/// * `return` 本地时间文本。
pub fn time_to_string(value: u64) -> String {
    chrono::TimeZone::from_utc_datetime(
        &chrono::Local,
        &chrono::NaiveDateTime::from_timestamp_millis(value as i64).unwrap(),
    )
    .format("%Y-%m-%d %H:%M:%S")
    .to_string()
}

/// 获取指定范围的 k 线数据。
/// 新的数据在前面。
///
/// * `product` 交易产品，例如，现货 BTC-USDT，合约 BTC-USDT-SWAP。
/// * `level` 时间级别。
/// * `range` 时间范围，0 表示获取所有数据，a..b 表示时间戳 a 到时间戳 b 范围之内的数据，
/// * `return` K 线数组。
pub async fn get_k_range<E, S, T>(
    exchange: &E,
    product: S,
    level: Level,
    range: T,
) -> anyhow::Result<Vec<K>>
where
    E: Exchange,
    S: AsRef<str>,
    T: Into<TimeRange>,
{
    let product = product.as_ref();
    let range = range.into();

    let mut result = Vec::new();

    if range.start == 0 && range.end == 0 {
        let mut time = 0;

        loop {
            let v = exchange.get_k(product, level, time).await?;

            if let Some(k) = v.last() {
                time = k.time;
                result.extend(v);
            } else {
                break;
            }
        }

        return Ok(result);
    }

    let mut end = range.end;

    if end == u64::MAX - 1 {
        end = std::time::SystemTime::now()
            .duration_since(std::time::SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
    }

    loop {
        let v = exchange.get_k(product, level, end).await?;

        if let Some(k) = v.last() {
            if k.time < range.start {
                for i in v {
                    if i.time >= range.start {
                        result.push(i);
                    }
                }
                break;
            }

            end = k.time;
            result.extend(v);
        } else {
            break;
        }
    }

    Ok(result)
}

/// 交易产品映射。
/// BTC-USDT <-> BTCUSDT。
/// BTC-USDT-SWAP <-> BTCUSDTSWAP。
///
/// * `value` 交易产品，例如，现货 BTC-USDT，合约 BTC-USDT-SWAP。
/// * `return` 映射值。
pub fn product_mapping<S>(value: S) -> std::borrow::Cow<'static, str>
where
    S: AsRef<str>,
{
    let result = match value.as_ref() {
        "MDT-USDT" => "MDTUSDT",
        "MDT-USDT-SWAP" => "MDTUSDTSWAP",
        "KNC-USDT" => "KNCUSDT",
        "KNC-USDT-SWAP" => "KNCUSDTSWAP",
        "CVX-USDT" => "CVXUSDT",
        "CVX-USDT-SWAP" => "CVXUSDTSWAP",
        "AGLD-USDT" => "AGLDUSDT",
        "AGLD-USDT-SWAP" => "AGLDUSDTSWAP",
        "IOTA-USDT" => "IOTAUSDT",
        "IOTA-USDT-SWAP" => "IOTAUSDTSWAP",
        "QTUM-USDT" => "QTUMUSDT",
        "QTUM-USDT-SWAP" => "QTUMUSDTSWAP",
        "AXS-USDT" => "AXSUSDT",
        "AXS-USDT-SWAP" => "AXSUSDTSWAP",
        "LQTY-USDT" => "LQTYUSDT",
        "LQTY-USDT-SWAP" => "LQTYUSDTSWAP",
        "ETC-USDT" => "ETCUSDT",
        "ETC-USDT-SWAP" => "ETCUSDTSWAP",
        "ASTR-USDT" => "ASTRUSDT",
        "ASTR-USDT-SWAP" => "ASTRUSDTSWAP",
        "BCH-USDT" => "BCHUSDT",
        "BCH-USDT-SWAP" => "BCHUSDTSWAP",
        "APT-USDT" => "APTUSDT",
        "APT-USDT-SWAP" => "APTUSDTSWAP",
        "TRX-USDT" => "TRXUSDT",
        "TRX-USDT-SWAP" => "TRXUSDTSWAP",
        "CELR-USDT" => "CELRUSDT",
        "CELR-USDT-SWAP" => "CELRUSDTSWAP",
        "CELO-USDT" => "CELOUSDT",
        "CELO-USDT-SWAP" => "CELOUSDTSWAP",
        "SAND-USDT" => "SANDUSDT",
        "SAND-USDT-SWAP" => "SANDUSDTSWAP",
        "KLAY-USDT" => "KLAYUSDT",
        "KLAY-USDT-SWAP" => "KLAYUSDTSWAP",
        "T-USDT" => "TUSDT",
        "T-USDT-SWAP" => "TUSDTSWAP",
        "FLM-USDT" => "FLMUSDT",
        "FLM-USDT-SWAP" => "FLMUSDTSWAP",
        "RAY-USDT" => "RAYUSDT",
        "RAY-USDT-SWAP" => "RAYUSDTSWAP",
        "SOL-USDT" => "SOLUSDT",
        "SOL-USDT-SWAP" => "SOLUSDTSWAP",
        "API3-USDT" => "API3USDT",
        "API3-USDT-SWAP" => "API3USDTSWAP",
        "YFI-USDT" => "YFIUSDT",
        "YFI-USDT-SWAP" => "YFIUSDTSWAP",
        "LDO-USDT" => "LDOUSDT",
        "LDO-USDT-SWAP" => "LDOUSDTSWAP",
        "NMR-USDT" => "NMRUSDT",
        "NMR-USDT-SWAP" => "NMRUSDTSWAP",
        "AAVE-USDT" => "AAVEUSDT",
        "AAVE-USDT-SWAP" => "AAVEUSDTSWAP",
        "TRB-USDT" => "TRBUSDT",
        "TRB-USDT-SWAP" => "TRBUSDTSWAP",
        "MATIC-USDT" => "MATICUSDT",
        "MATIC-USDT-SWAP" => "MATICUSDTSWAP",
        "DOGE-USDT" => "DOGEUSDT",
        "DOGE-USDT-SWAP" => "DOGEUSDTSWAP",
        "XRP-USDT" => "XRPUSDT",
        "XRP-USDT-SWAP" => "XRPUSDTSWAP",
        "ENS-USDT" => "ENSUSDT",
        "ENS-USDT-SWAP" => "ENSUSDTSWAP",
        "LPT-USDT" => "LPTUSDT",
        "LPT-USDT-SWAP" => "LPTUSDTSWAP",
        "BNB-USDT" => "BNBUSDT",
        "BNB-USDT-SWAP" => "BNBUSDTSWAP",
        "SPELL-USDT" => "SPELLUSDT",
        "SPELL-USDT-SWAP" => "SPELLUSDTSWAP",
        "CRV-USDT" => "CRVUSDT",
        "CRV-USDT-SWAP" => "CRVUSDTSWAP",
        "ARB-USDT" => "ARBUSDT",
        "ARB-USDT-SWAP" => "ARBUSDTSWAP",
        "EGLD-USDT" => "EGLDUSDT",
        "EGLD-USDT-SWAP" => "EGLDUSDTSWAP",
        "NEO-USDT" => "NEOUSDT",
        "NEO-USDT-SWAP" => "NEOUSDTSWAP",
        "CHZ-USDT" => "CHZUSDT",
        "CHZ-USDT-SWAP" => "CHZUSDTSWAP",
        "USDC-USDT" => "USDCUSDT",
        "USDC-USDT-SWAP" => "USDCUSDTSWAP",
        "DOT-USDT" => "DOTUSDT",
        "DOT-USDT-SWAP" => "DOTUSDTSWAP",
        "MINA-USDT" => "MINAUSDT",
        "MINA-USDT-SWAP" => "MINAUSDTSWAP",
        "EOS-USDT" => "EOSUSDT",
        "EOS-USDT-SWAP" => "EOSUSDTSWAP",
        "GMX-USDT" => "GMXUSDT",
        "GMX-USDT-SWAP" => "GMXUSDTSWAP",
        "GRT-USDT" => "GRTUSDT",
        "GRT-USDT-SWAP" => "GRTUSDTSWAP",
        "SKL-USDT" => "SKLUSDT",
        "SKL-USDT-SWAP" => "SKLUSDTSWAP",
        "IOST-USDT" => "IOSTUSDT",
        "IOST-USDT-SWAP" => "IOSTUSDTSWAP",
        "RDNT-USDT" => "RDNTUSDT",
        "RDNT-USDT-SWAP" => "RDNTUSDTSWAP",
        "BTC-USDT" => "BTCUSDT",
        "BTC-USDT-SWAP" => "BTCUSDTSWAP",
        "WOO-USDT" => "WOOUSDT",
        "WOO-USDT-SWAP" => "WOOUSDTSWAP",
        "ACH-USDT" => "ACHUSDT",
        "ACH-USDT-SWAP" => "ACHUSDTSWAP",
        "APE-USDT" => "APEUSDT",
        "APE-USDT-SWAP" => "APEUSDTSWAP",
        "ID-USDT" => "IDUSDT",
        "ID-USDT-SWAP" => "IDUSDTSWAP",
        "ADA-USDT" => "ADAUSDT",
        "ADA-USDT-SWAP" => "ADAUSDTSWAP",
        "HNT-USDT" => "HNTUSDT",
        "HNT-USDT-SWAP" => "HNTUSDTSWAP",
        "ALPHA-USDT" => "ALPHAUSDT",
        "ALPHA-USDT-SWAP" => "ALPHAUSDTSWAP",
        "CFX-USDT" => "CFXUSDT",
        "CFX-USDT-SWAP" => "CFXUSDTSWAP",
        "SRM-USDT" => "SRMUSDT",
        "SRM-USDT-SWAP" => "SRMUSDTSWAP",
        "UNI-USDT" => "UNIUSDT",
        "UNI-USDT-SWAP" => "UNIUSDTSWAP",
        "THETA-USDT" => "THETAUSDT",
        "THETA-USDT-SWAP" => "THETAUSDTSWAP",
        "HBAR-USDT" => "HBARUSDT",
        "HBAR-USDT-SWAP" => "HBARUSDTSWAP",
        "ZEC-USDT" => "ZECUSDT",
        "ZEC-USDT-SWAP" => "ZECUSDTSWAP",
        "SUSHI-USDT" => "SUSHIUSDT",
        "SUSHI-USDT-SWAP" => "SUSHIUSDTSWAP",
        "LTC-USDT" => "LTCUSDT",
        "LTC-USDT-SWAP" => "LTCUSDTSWAP",
        "ICX-USDT" => "ICXUSDT",
        "ICX-USDT-SWAP" => "ICXUSDTSWAP",
        "LINK-USDT" => "LINKUSDT",
        "LINK-USDT-SWAP" => "LINKUSDTSWAP",
        "XTZ-USDT" => "XTZUSDT",
        "XTZ-USDT-SWAP" => "XTZUSDTSWAP",
        "RVN-USDT" => "RVNUSDT",
        "RVN-USDT-SWAP" => "RVNUSDTSWAP",
        "WLD-USDT" => "WLDUSDT",
        "WLD-USDT-SWAP" => "WLDUSDTSWAP",
        "OP-USDT" => "OPUSDT",
        "OP-USDT-SWAP" => "OPUSDTSWAP",
        "REN-USDT" => "RENUSDT",
        "REN-USDT-SWAP" => "RENUSDTSWAP",
        "BLUR-USDT" => "BLURUSDT",
        "BLUR-USDT-SWAP" => "BLURUSDTSWAP",
        "SUI-USDT" => "SUIUSDT",
        "SUI-USDT-SWAP" => "SUIUSDTSWAP",
        "ICP-USDT" => "ICPUSDT",
        "ICP-USDT-SWAP" => "ICPUSDTSWAP",
        "XMR-USDT" => "XMRUSDT",
        "XMR-USDT-SWAP" => "XMRUSDTSWAP",
        "ZEN-USDT" => "ZENUSDT",
        "ZEN-USDT-SWAP" => "ZENUSDTSWAP",
        "FTM-USDT" => "FTMUSDT",
        "FTM-USDT-SWAP" => "FTMUSDTSWAP",
        "MAGIC-USDT" => "MAGICUSDT",
        "MAGIC-USDT-SWAP" => "MAGICUSDTSWAP",
        "DGB-USDT" => "DGBUSDT",
        "DGB-USDT-SWAP" => "DGBUSDTSWAP",
        "LRC-USDT" => "LRCUSDT",
        "LRC-USDT-SWAP" => "LRCUSDTSWAP",
        "DYDX-USDT" => "DYDXUSDT",
        "DYDX-USDT-SWAP" => "DYDXUSDTSWAP",
        "ZRX-USDT" => "ZRXUSDT",
        "ZRX-USDT-SWAP" => "ZRXUSDTSWAP",
        "SC-USDT" => "SCUSDT",
        "SC-USDT-SWAP" => "SCUSDTSWAP",
        "FIL-USDT" => "FILUSDT",
        "FIL-USDT-SWAP" => "FILUSDTSWAP",
        "RSR-USDT" => "RSRUSDT",
        "RSR-USDT-SWAP" => "RSRUSDTSWAP",
        "ETH-BTC" => "ETHBTC",
        "ETH-BTC-SWAP" => "ETHBTCSWAP",
        "ONT-USDT" => "ONTUSDT",
        "ONT-USDT-SWAP" => "ONTUSDTSWAP",
        "FXS-USDT" => "FXSUSDT",
        "FXS-USDT-SWAP" => "FXSUSDTSWAP",
        "UMA-USDT" => "UMAUSDT",
        "UMA-USDT-SWAP" => "UMAUSDTSWAP",
        "AR-USDT" => "ARUSDT",
        "AR-USDT-SWAP" => "ARUSDTSWAP",
        "BAND-USDT" => "BANDUSDT",
        "BAND-USDT-SWAP" => "BANDUSDTSWAP",
        "XLM-USDT" => "XLMUSDT",
        "XLM-USDT-SWAP" => "XLMUSDTSWAP",
        "SNX-USDT" => "SNXUSDT",
        "SNX-USDT-SWAP" => "SNXUSDTSWAP",
        "ATOM-USDT" => "ATOMUSDT",
        "ATOM-USDT-SWAP" => "ATOMUSDTSWAP",
        "BAT-USDT" => "BATUSDT",
        "BAT-USDT-SWAP" => "BATUSDTSWAP",
        "MANA-USDT" => "MANAUSDT",
        "MANA-USDT-SWAP" => "MANAUSDTSWAP",
        "CVC-USDT" => "CVCUSDT",
        "CVC-USDT-SWAP" => "CVCUSDTSWAP",
        "XEM-USDT" => "XEMUSDT",
        "XEM-USDT-SWAP" => "XEMUSDTSWAP",
        "SSV-USDT" => "SSVUSDT",
        "SSV-USDT-SWAP" => "SSVUSDTSWAP",
        "KSM-USDT" => "KSMUSDT",
        "KSM-USDT-SWAP" => "KSMUSDTSWAP",
        "JOE-USDT" => "JOEUSDT",
        "JOE-USDT-SWAP" => "JOEUSDTSWAP",
        "ETH-USDT" => "ETHUSDT",
        "ETH-USDT-SWAP" => "ETHUSDTSWAP",
        "STORJ-USDT" => "STORJUSDT",
        "STORJ-USDT-SWAP" => "STORJUSDTSWAP",
        "GMT-USDT" => "GMTUSDT",
        "GMT-USDT-SWAP" => "GMTUSDTSWAP",
        "OMG-USDT" => "OMGUSDT",
        "OMG-USDT-SWAP" => "OMGUSDTSWAP",
        "PEOPLE-USDT" => "PEOPLEUSDT",
        "PEOPLE-USDT-SWAP" => "PEOPLEUSDTSWAP",
        "BAL-USDT" => "BALUSDT",
        "BAL-USDT-SWAP" => "BALUSDTSWAP",
        "ZIL-USDT" => "ZILUSDT",
        "ZIL-USDT-SWAP" => "ZILUSDTSWAP",
        "FLOW-USDT" => "FLOWUSDT",
        "FLOW-USDT-SWAP" => "FLOWUSDTSWAP",
        "IMX-USDT" => "IMXUSDT",
        "IMX-USDT-SWAP" => "IMXUSDTSWAP",
        "COMP-USDT" => "COMPUSDT",
        "COMP-USDT-SWAP" => "COMPUSDTSWAP",
        "ALGO-USDT" => "ALGOUSDT",
        "ALGO-USDT-SWAP" => "ALGOUSDTSWAP",
        "WAVES-USDT" => "WAVESUSDT",
        "WAVES-USDT-SWAP" => "WAVESUSDTSWAP",
        "DASH-USDT" => "DASHUSDT",
        "DASH-USDT-SWAP" => "DASHUSDTSWAP",
        "ENJ-USDT" => "ENJUSDT",
        "ENJ-USDT-SWAP" => "ENJUSDTSWAP",
        "1INCH-USDT" => "1INCHUSDT",
        "1INCH-USDT-SWAP" => "1INCHUSDTSWAP",
        "PERP-USDT" => "PERPUSDT",
        "PERP-USDT-SWAP" => "PERPUSDTSWAP",
        "NEAR-USDT" => "NEARUSDT",
        "NEAR-USDT-SWAP" => "NEARUSDTSWAP",
        "ANT-USDT" => "ANTUSDT",
        "ANT-USDT-SWAP" => "ANTUSDTSWAP",
        "GAL-USDT" => "GALUSDT",
        "GAL-USDT-SWAP" => "GALUSDTSWAP",
        "ONE-USDT" => "ONEUSDT",
        "ONE-USDT-SWAP" => "ONEUSDTSWAP",
        "MKR-USDT" => "MKRUSDT",
        "MKR-USDT-SWAP" => "MKRUSDTSWAP",
        "GALA-USDT" => "GALAUSDT",
        "GALA-USDT-SWAP" => "GALAUSDTSWAP",
        "AVAX-USDT" => "AVAXUSDT",
        "AVAX-USDT-SWAP" => "AVAXUSDTSWAP",
        "MASK-USDT" => "MASKUSDT",
        "MASK-USDT-SWAP" => "MASKUSDTSWAP",
        "STX-USDT" => "STXUSDT",
        "STX-USDT-SWAP" => "STXUSDTSWAP",
        // ================================
        "MDTUSDT" => "MDT-USDT",
        "MDTUSDTSWAP" => "MDT-USDT-SWAP",
        "KNCUSDT" => "KNC-USDT",
        "KNCUSDTSWAP" => "KNC-USDT-SWAP",
        "CVXUSDT" => "CVX-USDT",
        "CVXUSDTSWAP" => "CVX-USDT-SWAP",
        "AGLDUSDT" => "AGLD-USDT",
        "AGLDUSDTSWAP" => "AGLD-USDT-SWAP",
        "IOTAUSDT" => "IOTA-USDT",
        "IOTAUSDTSWAP" => "IOTA-USDT-SWAP",
        "QTUMUSDT" => "QTUM-USDT",
        "QTUMUSDTSWAP" => "QTUM-USDT-SWAP",
        "AXSUSDT" => "AXS-USDT",
        "AXSUSDTSWAP" => "AXS-USDT-SWAP",
        "LQTYUSDT" => "LQTY-USDT",
        "LQTYUSDTSWAP" => "LQTY-USDT-SWAP",
        "ETCUSDT" => "ETC-USDT",
        "ETCUSDTSWAP" => "ETC-USDT-SWAP",
        "ASTRUSDT" => "ASTR-USDT",
        "ASTRUSDTSWAP" => "ASTR-USDT-SWAP",
        "BCHUSDT" => "BCH-USDT",
        "BCHUSDTSWAP" => "BCH-USDT-SWAP",
        "APTUSDT" => "APT-USDT",
        "APTUSDTSWAP" => "APT-USDT-SWAP",
        "TRXUSDT" => "TRX-USDT",
        "TRXUSDTSWAP" => "TRX-USDT-SWAP",
        "CELRUSDT" => "CELR-USDT",
        "CELRUSDTSWAP" => "CELR-USDT-SWAP",
        "CELOUSDT" => "CELO-USDT",
        "CELOUSDTSWAP" => "CELO-USDT-SWAP",
        "SANDUSDT" => "SAND-USDT",
        "SANDUSDTSWAP" => "SAND-USDT-SWAP",
        "KLAYUSDT" => "KLAY-USDT",
        "KLAYUSDTSWAP" => "KLAY-USDT-SWAP",
        "TUSDT" => "T-USDT",
        "TUSDTSWAP" => "T-USDT-SWAP",
        "FLMUSDT" => "FLM-USDT",
        "FLMUSDTSWAP" => "FLM-USDT-SWAP",
        "RAYUSDT" => "RAY-USDT",
        "RAYUSDTSWAP" => "RAY-USDT-SWAP",
        "SOLUSDT" => "SOL-USDT",
        "SOLUSDTSWAP" => "SOL-USDT-SWAP",
        "API3USDT" => "API3-USDT",
        "API3USDTSWAP" => "API3-USDT-SWAP",
        "YFIUSDT" => "YFI-USDT",
        "YFIUSDTSWAP" => "YFI-USDT-SWAP",
        "LDOUSDT" => "LDO-USDT",
        "LDOUSDTSWAP" => "LDO-USDT-SWAP",
        "NMRUSDT" => "NMR-USDT",
        "NMRUSDTSWAP" => "NMR-USDT-SWAP",
        "AAVEUSDT" => "AAVE-USDT",
        "AAVEUSDTSWAP" => "AAVE-USDT-SWAP",
        "TRBUSDT" => "TRB-USDT",
        "TRBUSDTSWAP" => "TRB-USDT-SWAP",
        "MATICUSDT" => "MATIC-USDT",
        "MATICUSDTSWAP" => "MATIC-USDT-SWAP",
        "DOGEUSDT" => "DOGE-USDT",
        "DOGEUSDTSWAP" => "DOGE-USDT-SWAP",
        "XRPUSDT" => "XRP-USDT",
        "XRPUSDTSWAP" => "XRP-USDT-SWAP",
        "ENSUSDT" => "ENS-USDT",
        "ENSUSDTSWAP" => "ENS-USDT-SWAP",
        "LPTUSDT" => "LPT-USDT",
        "LPTUSDTSWAP" => "LPT-USDT-SWAP",
        "BNBUSDT" => "BNB-USDT",
        "BNBUSDTSWAP" => "BNB-USDT-SWAP",
        "SPELLUSDT" => "SPELL-USDT",
        "SPELLUSDTSWAP" => "SPELL-USDT-SWAP",
        "CRVUSDT" => "CRV-USDT",
        "CRVUSDTSWAP" => "CRV-USDT-SWAP",
        "ARBUSDT" => "ARB-USDT",
        "ARBUSDTSWAP" => "ARB-USDT-SWAP",
        "EGLDUSDT" => "EGLD-USDT",
        "EGLDUSDTSWAP" => "EGLD-USDT-SWAP",
        "NEOUSDT" => "NEO-USDT",
        "NEOUSDTSWAP" => "NEO-USDT-SWAP",
        "CHZUSDT" => "CHZ-USDT",
        "CHZUSDTSWAP" => "CHZ-USDT-SWAP",
        "USDCUSDT" => "USDC-USDT",
        "USDCUSDTSWAP" => "USDC-USDT-SWAP",
        "DOTUSDT" => "DOT-USDT",
        "DOTUSDTSWAP" => "DOT-USDT-SWAP",
        "MINAUSDT" => "MINA-USDT",
        "MINAUSDTSWAP" => "MINA-USDT-SWAP",
        "EOSUSDT" => "EOS-USDT",
        "EOSUSDTSWAP" => "EOS-USDT-SWAP",
        "GMXUSDT" => "GMX-USDT",
        "GMXUSDTSWAP" => "GMX-USDT-SWAP",
        "GRTUSDT" => "GRT-USDT",
        "GRTUSDTSWAP" => "GRT-USDT-SWAP",
        "SKLUSDT" => "SKL-USDT",
        "SKLUSDTSWAP" => "SKL-USDT-SWAP",
        "IOSTUSDT" => "IOST-USDT",
        "IOSTUSDTSWAP" => "IOST-USDT-SWAP",
        "RDNTUSDT" => "RDNT-USDT",
        "RDNTUSDTSWAP" => "RDNT-USDT-SWAP",
        "BTCUSDT" => "BTC-USDT",
        "BTCUSDTSWAP" => "BTC-USDT-SWAP",
        "WOOUSDT" => "WOO-USDT",
        "WOOUSDTSWAP" => "WOO-USDT-SWAP",
        "ACHUSDT" => "ACH-USDT",
        "ACHUSDTSWAP" => "ACH-USDT-SWAP",
        "APEUSDT" => "APE-USDT",
        "APEUSDTSWAP" => "APE-USDT-SWAP",
        "IDUSDT" => "ID-USDT",
        "IDUSDTSWAP" => "ID-USDT-SWAP",
        "ADAUSDT" => "ADA-USDT",
        "ADAUSDTSWAP" => "ADA-USDT-SWAP",
        "HNTUSDT" => "HNT-USDT",
        "HNTUSDTSWAP" => "HNT-USDT-SWAP",
        "ALPHAUSDT" => "ALPHA-USDT",
        "ALPHAUSDTSWAP" => "ALPHA-USDT-SWAP",
        "CFXUSDT" => "CFX-USDT",
        "CFXUSDTSWAP" => "CFX-USDT-SWAP",
        "SRMUSDT" => "SRM-USDT",
        "SRMUSDTSWAP" => "SRM-USDT-SWAP",
        "UNIUSDT" => "UNI-USDT",
        "UNIUSDTSWAP" => "UNI-USDT-SWAP",
        "THETAUSDT" => "THETA-USDT",
        "THETAUSDTSWAP" => "THETA-USDT-SWAP",
        "HBARUSDT" => "HBAR-USDT",
        "HBARUSDTSWAP" => "HBAR-USDT-SWAP",
        "ZECUSDT" => "ZEC-USDT",
        "ZECUSDTSWAP" => "ZEC-USDT-SWAP",
        "SUSHIUSDT" => "SUSHI-USDT",
        "SUSHIUSDTSWAP" => "SUSHI-USDT-SWAP",
        "LTCUSDT" => "LTC-USDT",
        "LTCUSDTSWAP" => "LTC-USDT-SWAP",
        "ICXUSDT" => "ICX-USDT",
        "ICXUSDTSWAP" => "ICX-USDT-SWAP",
        "LINKUSDT" => "LINK-USDT",
        "LINKUSDTSWAP" => "LINK-USDT-SWAP",
        "XTZUSDT" => "XTZ-USDT",
        "XTZUSDTSWAP" => "XTZ-USDT-SWAP",
        "RVNUSDT" => "RVN-USDT",
        "RVNUSDTSWAP" => "RVN-USDT-SWAP",
        "WLDUSDT" => "WLD-USDT",
        "WLDUSDTSWAP" => "WLD-USDT-SWAP",
        "OPUSDT" => "OP-USDT",
        "OPUSDTSWAP" => "OP-USDT-SWAP",
        "RENUSDT" => "REN-USDT",
        "RENUSDTSWAP" => "REN-USDT-SWAP",
        "BLURUSDT" => "BLUR-USDT",
        "BLURUSDTSWAP" => "BLUR-USDT-SWAP",
        "SUIUSDT" => "SUI-USDT",
        "SUIUSDTSWAP" => "SUI-USDT-SWAP",
        "ICPUSDT" => "ICP-USDT",
        "ICPUSDTSWAP" => "ICP-USDT-SWAP",
        "XMRUSDT" => "XMR-USDT",
        "XMRUSDTSWAP" => "XMR-USDT-SWAP",
        "ZENUSDT" => "ZEN-USDT",
        "ZENUSDTSWAP" => "ZEN-USDT-SWAP",
        "FTMUSDT" => "FTM-USDT",
        "FTMUSDTSWAP" => "FTM-USDT-SWAP",
        "MAGICUSDT" => "MAGIC-USDT",
        "MAGICUSDTSWAP" => "MAGIC-USDT-SWAP",
        "DGBUSDT" => "DGB-USDT",
        "DGBUSDTSWAP" => "DGB-USDT-SWAP",
        "LRCUSDT" => "LRC-USDT",
        "LRCUSDTSWAP" => "LRC-USDT-SWAP",
        "DYDXUSDT" => "DYDX-USDT",
        "DYDXUSDTSWAP" => "DYDX-USDT-SWAP",
        "ZRXUSDT" => "ZRX-USDT",
        "ZRXUSDTSWAP" => "ZRX-USDT-SWAP",
        "SCUSDT" => "SC-USDT",
        "SCUSDTSWAP" => "SC-USDT-SWAP",
        "FILUSDT" => "FIL-USDT",
        "FILUSDTSWAP" => "FIL-USDT-SWAP",
        "RSRUSDT" => "RSR-USDT",
        "RSRUSDTSWAP" => "RSR-USDT-SWAP",
        "ETHBTC" => "ETH-BTC",
        "ETHBTCSWAP" => "ETH-BTC-SWAP",
        "ONTUSDT" => "ONT-USDT",
        "ONTUSDTSWAP" => "ONT-USDT-SWAP",
        "FXSUSDT" => "FXS-USDT",
        "FXSUSDTSWAP" => "FXS-USDT-SWAP",
        "UMAUSDT" => "UMA-USDT",
        "UMAUSDTSWAP" => "UMA-USDT-SWAP",
        "ARUSDT" => "AR-USDT",
        "ARUSDTSWAP" => "AR-USDT-SWAP",
        "BANDUSDT" => "BAND-USDT",
        "BANDUSDTSWAP" => "BAND-USDT-SWAP",
        "XLMUSDT" => "XLM-USDT",
        "XLMUSDTSWAP" => "XLM-USDT-SWAP",
        "SNXUSDT" => "SNX-USDT",
        "SNXUSDTSWAP" => "SNX-USDT-SWAP",
        "ATOMUSDT" => "ATOM-USDT",
        "ATOMUSDTSWAP" => "ATOM-USDT-SWAP",
        "BATUSDT" => "BAT-USDT",
        "BATUSDTSWAP" => "BAT-USDT-SWAP",
        "MANAUSDT" => "MANA-USDT",
        "MANAUSDTSWAP" => "MANA-USDT-SWAP",
        "CVCUSDT" => "CVC-USDT",
        "CVCUSDTSWAP" => "CVC-USDT-SWAP",
        "XEMUSDT" => "XEM-USDT",
        "XEMUSDTSWAP" => "XEM-USDT-SWAP",
        "SSVUSDT" => "SSV-USDT",
        "SSVUSDTSWAP" => "SSV-USDT-SWAP",
        "KSMUSDT" => "KSM-USDT",
        "KSMUSDTSWAP" => "KSM-USDT-SWAP",
        "JOEUSDT" => "JOE-USDT",
        "JOEUSDTSWAP" => "JOE-USDT-SWAP",
        "ETHUSDT" => "ETH-USDT",
        "ETHUSDTSWAP" => "ETH-USDT-SWAP",
        "STORJUSDT" => "STORJ-USDT",
        "STORJUSDTSWAP" => "STORJ-USDT-SWAP",
        "GMTUSDT" => "GMT-USDT",
        "GMTUSDTSWAP" => "GMT-USDT-SWAP",
        "OMGUSDT" => "OMG-USDT",
        "OMGUSDTSWAP" => "OMG-USDT-SWAP",
        "PEOPLEUSDT" => "PEOPLE-USDT",
        "PEOPLEUSDTSWAP" => "PEOPLE-USDT-SWAP",
        "BALUSDT" => "BAL-USDT",
        "BALUSDTSWAP" => "BAL-USDT-SWAP",
        "ZILUSDT" => "ZIL-USDT",
        "ZILUSDTSWAP" => "ZIL-USDT-SWAP",
        "FLOWUSDT" => "FLOW-USDT",
        "FLOWUSDTSWAP" => "FLOW-USDT-SWAP",
        "IMXUSDT" => "IMX-USDT",
        "IMXUSDTSWAP" => "IMX-USDT-SWAP",
        "COMPUSDT" => "COMP-USDT",
        "COMPUSDTSWAP" => "COMP-USDT-SWAP",
        "ALGOUSDT" => "ALGO-USDT",
        "ALGOUSDTSWAP" => "ALGO-USDT-SWAP",
        "WAVESUSDT" => "WAVES-USDT",
        "WAVESUSDTSWAP" => "WAVES-USDT-SWAP",
        "DASHUSDT" => "DASH-USDT",
        "DASHUSDTSWAP" => "DASH-USDT-SWAP",
        "ENJUSDT" => "ENJ-USDT",
        "ENJUSDTSWAP" => "ENJ-USDT-SWAP",
        "1INCHUSDT" => "1INCH-USDT",
        "1INCHUSDTSWAP" => "1INCH-USDT-SWAP",
        "PERPUSDT" => "PERP-USDT",
        "PERPUSDTSWAP" => "PERP-USDT-SWAP",
        "NEARUSDT" => "NEAR-USDT",
        "NEARUSDTSWAP" => "NEAR-USDT-SWAP",
        "ANTUSDT" => "ANT-USDT",
        "ANTUSDTSWAP" => "ANT-USDT-SWAP",
        "GALUSDT" => "GAL-USDT",
        "GALUSDTSWAP" => "GAL-USDT-SWAP",
        "ONEUSDT" => "ONE-USDT",
        "ONEUSDTSWAP" => "ONE-USDT-SWAP",
        "MKRUSDT" => "MKR-USDT",
        "MKRUSDTSWAP" => "MKR-USDT-SWAP",
        "GALAUSDT" => "GALA-USDT",
        "GALAUSDTSWAP" => "GALA-USDT-SWAP",
        "AVAXUSDT" => "AVAX-USDT",
        "AVAXUSDTSWAP" => "AVAX-USDT-SWAP",
        "MASKUSDT" => "MASK-USDT",
        "MASKUSDTSWAP" => "MASK-USDT-SWAP",
        "STXUSDT" => "STX-USDT",
        "STXUSDTSWAP" => "STX-USDT-SWAP",
        _ => "",
    };

    if result.is_empty() {
        std::borrow::Cow::Owned(if result.contains("-") {
            result.replace("-", "")
        } else if result.ends_with("SWAP") {
            result.replace("USDT", "-USDT-")
        } else {
            result.replace("USDT", "-USDT")
        })
    } else {
        std::borrow::Cow::Borrowed(result)
    }
}
