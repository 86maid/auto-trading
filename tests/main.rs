use auto_trading::*;

#[tokio::test]
async fn test_1() {
    let exchange = Okx::new().unwrap();

    let config = Config::new()
        .initial_margin(1000.0)
        .quantity(Unit::Quantity(0.01))
        // .margin(Unit::Quantity(10.0))
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
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64
                - 1000 * 60 * 60 * 24 * 7 * 4..,
        )
        .await
        .unwrap();

    println!("历史仓位 {:#?}", result);
    println!("所有盈亏 {}", result.iter().map(|v| v.profit).sum::<f64>());
}

#[tokio::test]
async fn test_2() {
    // 使用 1 分钟的 k 线数据。
    let k = serde_json::from_str::<Vec<K>>(include_str!("../BTC-USDT-SWAP-1m.json")).unwrap();

    let exchange = LocalExchange::new()
        .push("BTC-USDT-SWAP", Level::Minute1, k.clone(), 0.01, 0.0)
        .push(
            "BTC-USDT-SWAP",
            Level::Hour4,
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

#[tokio::test]
async fn test_local() {
    // 使用本地交易所
    let exchange = LocalExchange::new().push(
        "BTC-USDT-SWAP",
        Level::Hour4,
        serde_json::from_str(include_str!("../BTC-USDT-SWAP-4h.json")).unwrap(),
        0.01,
        0.0,
    );

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
            0,
        )
        .await
        .unwrap();

    println!("历史仓位 {:#?}", result);
    println!("所有盈亏 {}", result.iter().map(|v| v.profit).sum::<f64>());
}

#[tokio::test]
async fn test_my() {
    let k = serde_json::from_str::<Vec<K>>(include_str!("../BTC-USDT-SWAP-1m.json")).unwrap();
    let k15 = k_convert(&k, Level::Minute15);
    let k4 = k_convert(&k, Level::Hour4);

    let exchange = LocalExchange::new()
        .push("BTC-USDT-SWAP", Level::Minute1, k.clone(), 0.01, 0.0)
        .push("BTC-USDT-SWAP", Level::Minute15, k15.clone(), 0.01, 0.0)
        .push("BTC-USDT-SWAP", Level::Hour4, k4.clone(), 0.01, 0.0);

    let config = Config::new()
        .initial_margin(1000.0)
        .quantity(Unit::Quantity(0.01))
        // .margin(Unit::Quantity(10.0))
        .lever(100)
        .open_fee(0.0002)
        .close_fee(0.0005)
        .maintenance(0.004);

    let backtester = Backtester::new(exchange, config);

    let strategy = |cx: &mut Context| {
        // if cx.position().is_some() {
        //     // 当前有仓位就不要判断了
        //     return;
        // }

        if cx.open[2] < cx.close[2]
            && cx.open[1] < cx.close[1]
            && cx.open[0] < cx.close[0]
            && cx.open[2] < cx.open[1]
            && cx.open[1] < cx.open[0]
            || cx.open[2] > cx.close[2]
                && cx.open[1] > cx.close[1]
                && cx.open[0] > cx.close[0]
                && cx.open[2] > cx.open[1]
                && cx.open[1] > cx.open[0]
        {
            fn check(arr: &[f64], epsilon: f64) -> bool {
                if arr.is_empty() {
                    return false;
                }
                let first_element = arr[0];
                for element in arr.iter().skip(1) {
                    if (element - first_element).abs() > epsilon {
                        return false;
                    }
                }
                true
            }
            let a = cx.high[2] - cx.low[2];
            let b = cx.high[1] - cx.low[1];
            let c = cx.high[0] - cx.low[0];
            if check(&[a, b, c], 20.0) {
                if cx.open[2] < cx.close[2] {
                    match cx.order_profit_loss(
                        Side::BuyLong,
                        cx.close[0],
                        Unit::Proportion(0.05),
                        Unit::Quantity(cx.open[2]),
                    ) {
                        Ok(v) => {
                            println!(
                                "{} 做多 {:?}",
                                time_to_string(cx.time),
                                cx.delegate(v).unwrap()
                            );
                        }
                        Err(v) => {
                            println!("{} 做多 {}", time_to_string(cx.time), v);
                        }
                    }
                } else {
                    // match cx.order_profit_loss(
                    //     Side::SellShort,
                    //     cx.close[0],
                    //     Unit::Proportion(0.03),
                    //     Unit::Ignore,
                    // ) {
                    //     Ok(v) => {
                    //         println!(
                    //             "{} 做空 {:?}",
                    //             time_to_string(cx.time),
                    //             cx.delegate(v).unwrap()
                    //         );
                    //     }
                    //     Err(v) => {
                    //         println!("{} 做空 {}", time_to_string(cx.time), v);
                    //     }
                    // };
                }
            };
        }
    };

    let result = backtester
        .start_amplifier(
            strategy,
            "BTC-USDT-SWAP",
            Level::Minute1,
            Level::Minute15,
            1636603200000..,
        )
        .await
        .unwrap();

    let mut q = 0.0;
    let mut w = 0.0;
    let mut a = 0;
    let mut b = 0;
    result.iter().for_each(|v| {
        if v.profit > 0.0 {
            a += 1;
            q += v.profit;
        } else if v.profit < 0.0 {
            b += 1;
            w += v.profit.abs();
        }
    });

    println!("所有盈亏 {}", result.iter().map(|v| v.profit).sum::<f64>());
    println!("盈亏比 {}", q / w);
    println!("胜率 {}", a as f64 / (a + b) as f64);

    std::fs::write("./仓位记录.txt", format!("{:#?}", result)).unwrap();
    std::fs::write("./index.html", to_html(k15, result)).unwrap();
}
