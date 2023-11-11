# Auto Trading

[<img alt="github" src="https://img.shields.io/badge/github-86maid/auto--trading- ?logo=github" height="20">](https://github.com/86maid/auto-trading)
[![Latest version](https://img.shields.io/crates/v/auto-trading)](https://crates.io/crates/auto-trading)
[![Documentation](https://docs.rs/auto-trading/badge.svg)](https://docs.rs/auto-trading)
[![Apache](https://img.shields.io/badge/license-Apache-blue.svg)](https://github.com/86maid/auto-trading/blob/master/LICENSE)

回测，策略，多平台，量化交易框架。

backtest, strategy, multiple platforms, quantitative trading framework.

# Dependencies

```
[dependencies]
auto-trading = "0.8.5"
```

# Examples 1

使用欧易交易所进行回测。

Perform backtesting using the Okx exchange.

* `product`: BTC-USDT-SWAP  
* `level`: Hour4  
* `range`: 1692963462000..  
* `buy`: cci(close, 20) <= -350
* `sell`: cci(close, 20) >= 100

```rust
use auto_trading::*;

#[tokio::test]
async fn test_1() {
    let exchange = Okx::new().unwrap();

    let config = Config::new()
        .initial_margin(1000.0)
        .quantity(Unit::Quantity(0.01))
        .margin(Unit::Quantity(10.0))
        .lever(100)
        .open_fee(0.0002)
        .close_fee(0.0005)
        .maintenance(0.004);

    let backtester = Backtester::new(exchange, config);

    let result = backtester
        .start(
            |cx| {
                if cx.position().is_none() {
                    if cci(cx.close, 20) <= -350.0 {
                        let result = cx.order(Side::BuyLong, 0.0);
                        println!(
                            "开仓委托结果 {} {} {:?}",
                            time_to_string(cx.time),
                            cx.close,
                            result
                        );
                    }
                } else {
                    if cci(cx.close, 20) >= 100.0 {
                        let result = cx.order(Side::BuySell, 0.0);
                        println!(
                            "平仓委托结果 {} {} {:?}",
                            time_to_string(cx.time),
                            cx.close,
                            result
                        );
                    }
                }
            },
            "BTC-USDT-SWAP",
            Level::Hour4,
            1692963462000..,
        )
        .await
        .unwrap();

    println!("历史仓位 {:#?}", result);
    println!("所有盈亏 {}", result.iter().map(|v| v.profit).sum::<f64>());
}
```

使用币安交易所只需要做简单的修改。

Making modifications to use the Binance exchange is a straightforward process.

```rust
let exchange = Binance::new().unwrap();
```

使用本地交易所，从文件获取读取 k 线数据。

Using a local exchange, retrieve candlestick (k-line) data from a file.

```rust
let exchange = LocalExchange::new().push(
    "BTC-USDT-SWAP",
    Level::Hour4,
    serde_json::from_str(include_str!("BTC-USDT-SWAP-4h.json")).unwrap(),
    0.01,
    0.0,
);
```

更多的委托参数。

More delegate parameters.

```rust
cx.order_condition(
    side,
    price,
    quantity,
    margin,
    stop_profit_condition,
    stop_loss_condition,
    stop_profit,
    stop_loss,
);
```

# Examples 2

使用 1 分钟时间级别的 k 线数据在 4 小时的策略上回测，你的强平，平仓，开仓，盈亏会按照 1 分钟的时间级别刷新，而策略的调用周期为 4 小时。

Backtesting a 4-hour strategy using 1-minute candlestick data means that your liquidation, closing, opening, and profit/loss calculations will refresh at the 1-minute time interval, while the strategy's invocation period is set at 4 hours.

## 强烈建议使用 1 分钟级别的 k 线数据进行回测。

## Backtesting with 1-minute level k-line data is highly recommended.

```rust
use auto_trading::*;

#[tokio::test]
async fn test_2() {
    // 使用 1 分钟的 k 线数据。
    let k = serde_json::from_str::<Vec<K>>(include_str!("../BTC-USDT-SWAP-1m.json")).unwrap();

    let exchange = LocalExchange::new()
        .push("BTC-USDT-SWAP", Level::Minute1, k.clone(), 0.01, 0.0)
        .push(
            "BTC-USDT-SWAP",
            Level::Hour4,
            // k 线转换
            k_convert(k, Level::Hour4),
            0.01,
            0.0,
        );

    // Level::Minute1 -> Level::Hour4
    Backtester::new(exchange, Config::new())
        .start_amplifier(
            |cx| println!("{} {}", cx.time, time_to_string(cx.time)),
            "BTC-USDT-SWAP",
            Level::Minute1,
            Level::Hour4,
            0,
        )
        .await
        .unwrap();
}
```

# Built-in Functions

在 `auto_trading::util` 内置了 crossover, crossunder, highest, lowest, sma, ema, rma, cci, macd 等其他函数。

In `auto_trading::util`, there are other built-in functions such as crossover, crossunder, highest, lowest, sma, ema, rma, cci, macd, and more.

```rust
use auto_trading::*;

#[tokio::test]
async fn test_3() {
    println!("{}", time_to_string(1145141919810));
    println!("{}", string_to_time("2006-04-16 06:58:39"));
    println!(
        "{:?}",
        get_k_range(
            &Okx::new().unwrap(),
            "BTC-USDT-SWAP",
            Level::Hour4,
            1695212739000..1695644739000
        )
        .await
        .unwrap()
    );
}
```

# Data series

策略的 cx.open, cx.high, cx.low, cx.close 的数据类型为 `auto_trading::base::Source`，它不会因为越界而发生 panic。

The data types for `cx.open`, `cx.high`, `cx.low`, and `cx.close` in the strategy are `auto_trading::base::Source`, and they won't panic due to index out of bounds errors.

```rust
#[test]
fn test_4() {
    let array = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0];
    let source = Source::new(&array);
    assert!(source == 1.0);
    assert!(source[2] == 3.0);
    assert!(source[10].is_nan());
    assert!((&source[1..4]) == &[2.0, 3.0, 4.0][..]);
    assert!((&source[10..]).len() == 0);
}
```

# Visualize

使用 `to_html` 函数将回测结果可视化。

Use the `to_html` function to visualize the backtest results.

<img src="https://f.pz.al/pzal/2023/10/24/8414cbc43788e.png">

# Architecture

* `exchange` 交易所。
* `config` 交易配置。
* `backtester` 回测器。
* `match engine` 撮合引擎。
* `strategy` 策略。

```
                                          +========+
                                          | config |
                                          +========+
                                              ||
                                              ||
+================+                            vv
| okx            |     +==========+     +============+     +==========+
| binance        | --> | exchange | --> | backtester | <-- | strategy |
| local exchange |     +==========+     +============+     +==========+
+================+                            ^|
                                              ||
                                              |v
                                       +==============+
                                       | match engine |
                                       +==============+
```