use crate::*;

/// 信息。
#[derive(Debug)]
struct Message {
    /// 最小委托数量。
    min_size: f64,

    /// 最小名义价值。
    min_notional: f64,

    /// K 线数据。
    k: K,

    /// 委托 id，委托状态。
    delegate: Vec<(u64, DelegateState)>,

    /// 仓位。
    position: Option<Position>,
}

/// 撮合引擎。
#[derive(Debug)]
pub struct MatchEngine {
    /// 余额。
    balance: f64,

    /// 委托 id。
    id: u64,

    /// 交易配置。
    config: Config,

    /// 产品，信息。
    product: Vec<(String, Message)>,

    /// 历史仓位。
    history: Vec<Position>,
}

impl MatchEngine {
    /// 构造撮合引擎。
    ///
    /// * `config` 交易配置。
    pub fn new(config: Config) -> Self {
        Self {
            balance: config.initial_margin,
            id: 0,
            config,
            product: Vec::new(),
            history: Vec::new(),
        }
    }

    /// 获取余额。
    pub fn balance(&self) -> f64 {
        self.balance
    }

    /// 获取委托。
    ///
    /// * `product` 委托 id。
    /// * `return` 委托的状态，如果委托不存在或者已经成交，则返回 None。
    pub fn delegate(&self, id: u64) -> Option<DelegateState> {
        for i in self.product.iter() {
            if let Some(v) = i.1.delegate.iter().find(|v| v.0 == id).map(|v| v.1) {
                return Some(v);
            }
        }

        None
    }

    /// 获取当前仓位。
    ///
    /// * `product` 交易产品，例如，现货 BTC-USDT，合约 BTC-USDT-SWAP。
    /// * `return` 仓位。
    pub fn position<S>(&self, product: S) -> Option<&Position>
    where
        S: AsRef<str>,
    {
        let product = product.as_ref();
        self.product
            .iter()
            .find(|v| v.0 == product)
            .map(|v| &v.1.position)
            .and_then(|v| v.as_ref())
    }

    /// 获取历史仓位。
    pub fn history(&self) -> &Vec<Position> {
        &self.history
    }

    /// 插入产品。
    ///
    /// * `product` 交易产品。
    /// * `min_size` 最小委托数量。
    /// * `min_notional` 最小名义价值。
    pub fn insert_product<S>(&mut self, product: S, min_size: f64, min_notional: f64)
    where
        S: AsRef<str>,
    {
        let product = product.as_ref();

        let message = Message {
            min_size,
            min_notional,
            k: K {
                time: 114514,
                open: 0.0,
                high: 0.0,
                low: 0.0,
                close: 0.0,
            },
            delegate: Vec::new(),
            position: None,
        };

        if let Some(v) = self.product.iter().position(|v| v.0 == product) {
            self.product[v].1 = message;
        } else {
            self.product.push((product.to_string(), message));
        }
    }

    /// 删除产品。
    ///
    /// * `product` 交易产品。
    pub fn remove_product<S>(&mut self, product: S)
    where
        S: AsRef<str>,
    {
        let product = product.as_ref();
        self.product.retain(|v| v.0 != product);
    }

    /// 准备。
    /// 在调用委托之前，需要准备。
    /// 在准备之前，需要插入产品。
    ///
    /// * `product` 交易产品。
    /// * `k` k 线数据。
    pub fn ready<S>(&mut self, product: S, k: K)
    where
        S: AsRef<str>,
    {
        let product = product.as_ref();
        self.product
            .iter_mut()
            .find(|v| v.0 == product)
            .map(|v| &mut v.1)
            .expect(&format!("no product: {}", product))
            .k = k;
    }

    /// 委托。
    /// 如果做多限价大于市价，那么价格大于等于限价的时候才会成交。
    /// 如果做空限价小于市价，那么价格小于等于限价的时候才会成交。
    /// 如果平多限价小于市价，那么价格小于等于限价的时候才会成交。
    /// 如果平空限价大于市价，那么价格大于等于限价的时候才会成交。
    /// 做多的止盈触发价不能小于等于委托价格。
    /// 做空的止盈触发价不能大于等于委托价格。
    /// 做多的止损触发价不能大于等于委托价格。
    /// 做空的止损触发价不能小于等于委托价格。
    /// 限价平仓委托不会在当前 k 线被成交。
    /// 平仓不会导致仓位反向开单，平仓数量只能小于等于现有持仓数量。
    /// 如果在进行平仓操作后，现有的限价平仓委托的平仓量小于持仓量，则该委托将被撤销。
    /// 平仓的止盈止损无效。
    ///
    /// * `product` 交易产品，例如，现货 BTC-USDT，合约 BTC-USDT-SWAP。
    /// * `side` 委托方向。
    /// * `price` 委托价格，0 表示市价，其他表示限价。
    /// * `quantity` 委托数量，单位为币，如果是开仓，则 [`Unit::Ignore`] 表示使用 [`Config::quantity`] 的设置，如果是平仓，则 [`Unit::Ignore`] 表示全部仓位，[`Unit::Proportion`] 表示占用仓位的比例。
    /// * `margin` 保证金，[`Unit::Ignore`] 表示使用 [`Config::margin`] 的设置，保证金乘以杠杆必须大于仓位价值，即 [`Config::margin`] * [`Config::lever`] >= [`Config::quantity`]，超出仓位价值部分的保证金当作追加保证金。
    /// * `stop_profit_condition` 止盈触发价格，[`Unit::Ignore`] 表示不设置，且 `stop_profit` 无效。
    /// * `stop_loss_condition` 止损触发价格，[`Unit::Ignore`] 表示不设置，且 `stop_loss` 无效。
    /// * `stop_profit` 止盈委托价格，[`Unit::Ignore`] 表示不设置，其他表示限价。
    /// * `stop_loss` 止损委托格，[`Unit::Ignore`] 表示不设置，其他表示限价。
    /// * `return` 委托 id。
    pub fn order<S>(
        &mut self,
        product: S,
        side: Side,
        price: f64,
        quantity: Unit,
        margin: Unit,
        stop_profit_condition: Unit,
        stop_loss_condition: Unit,
        stop_profit: Unit,
        stop_loss: Unit,
    ) -> anyhow::Result<u64>
    where
        S: AsRef<str>,
    {
        let product = product.as_ref();

        let Message {
            min_size,
            min_notional,
            k,
            delegate,
            position,
        } = self
            .product
            .iter_mut()
            .find(|v| v.0 == product)
            .map(|v| &mut v.1)
            .ok_or(anyhow::anyhow!("no product: {}", product))?;

        if side == Side::BuyLong || side == Side::SellShort {
            // 市价转换
            let price = if price == 0.0 { k.close } else { price };

            // 委托数量
            let quantity = match if quantity == Unit::Ignore {
                self.config.quantity
            } else {
                quantity
            } {
                Unit::Ignore => *min_size,
                Unit::Quantity(v) => v,
                Unit::Proportion(v) => {
                    (self.config.initial_margin * v / price / *min_size).floor() * *min_size
                }
            };

            // 委托数量不能小于最小委托数量。
            if quantity < *min_size {
                anyhow::bail!(
                    "product {}: open quantity < min size: {} < {}",
                    product,
                    quantity,
                    min_size
                );
            }

            // 委托数量的价值
            let quantity_value = price * quantity;

            // 委托数量的价值不能小于最小名义价值
            if quantity_value < *min_notional {
                anyhow::bail!(
                    "product {}: open quantity value < min notional: {} < {}",
                    product,
                    quantity_value,
                    min_notional
                );
            }

            // 投入的保证金
            let margin = match if margin == Unit::Ignore {
                self.config.margin
            } else {
                margin
            } {
                Unit::Ignore => price * quantity / self.config.lever as f64,
                Unit::Quantity(v) => v,
                Unit::Proportion(v) => self.config.initial_margin * v,
            };

            // 保证金必须足够维持仓位价值
            // 写成乘法会有精度问题
            if margin < quantity_value / self.config.lever as f64 {
                anyhow::bail!(
                    "product {}: margin * lever < open quantity value: {} * {} < {}",
                    product,
                    margin,
                    self.config.lever,
                    quantity_value
                );
            }

            // 手续费
            let fee = price * quantity * self.config.open_fee;

            // 检查余额
            if self.balance < margin + fee {
                anyhow::bail!(
                    "product {}: insufficient fund: balance < margin + fee: {} < {} + {}",
                    product,
                    self.balance,
                    margin,
                    fee
                );
            }

            // 检查最大投入的保证金数量
            if self.config.max_margin != Unit::Ignore {
                if let Some(position) = position {
                    let max_margin = match self.config.max_margin {
                        Unit::Quantity(v) => v,
                        Unit::Proportion(v) => self.config.initial_margin * v,
                        _ => todo!("you are a big fool"),
                    };

                    if position.margin + margin > max_margin {
                        anyhow::bail!(
                            "product {}: position margin + open margin > max margin: {} + {} > {:?}",
                            product,
                            position.margin,
                            margin,
                            max_margin,
                        );
                    }
                }
            }

            // 检查止盈止损参数
            if stop_profit_condition == Unit::Ignore && stop_profit != Unit::Ignore {
                anyhow::bail!(
                    "product {}: stop profit must be zero, because stop profit condition is zero",
                    product
                );
            }

            if stop_loss_condition == Unit::Ignore && stop_loss != Unit::Ignore {
                anyhow::bail!(
                    "product {}: stop loss must be zero, because stop loss condition is zero",
                    product
                )
            }

            let stop_profit_condition = match stop_profit_condition {
                Unit::Ignore => Unit::Ignore,
                Unit::Quantity(v) => Unit::Quantity(v),
                Unit::Proportion(v) => Unit::Quantity(if side == Side::BuyLong {
                    price + price * v
                } else {
                    price - price * v
                }),
            };

            let stop_loss_condition = match stop_loss_condition {
                Unit::Ignore => Unit::Ignore,
                Unit::Quantity(v) => Unit::Quantity(v),
                Unit::Proportion(v) => Unit::Quantity(if side == Side::BuyLong {
                    price - price * v
                } else {
                    price + price * v
                }),
            };

            let stop_profit = match stop_profit {
                Unit::Ignore => Unit::Ignore,
                Unit::Quantity(v) => Unit::Quantity(v),
                Unit::Proportion(v) => Unit::Quantity(if side == Side::BuyLong {
                    price + price * v
                } else {
                    price - price * v
                }),
            };

            let stop_loss = match stop_loss {
                Unit::Ignore => Unit::Ignore,
                Unit::Quantity(v) => Unit::Quantity(v),
                Unit::Proportion(v) => Unit::Quantity(if side == Side::BuyLong {
                    price - price * v
                } else {
                    price + price * v
                }),
            };

            if let Unit::Quantity(v) = stop_profit_condition {
                if v <= 0.0 {
                    anyhow::bail!("product {}: stop profit condition invalid: {}", product, v)
                }
            }

            if let Unit::Quantity(v) = stop_loss_condition {
                if v <= 0.0 {
                    anyhow::bail!("product {}: stop loss condition invalid: {}", product, v)
                }
            }

            if let Unit::Quantity(v) = stop_profit {
                if v <= 0.0 {
                    anyhow::bail!("product {}: stop profit invalid: {}", product, v)
                }
            }

            if let Unit::Quantity(v) = stop_loss {
                if v <= 0.0 {
                    anyhow::bail!("product {}: stop loss invalid: {}", product, v)
                }
            }

            // 检查止盈止损是否有利于仓位
            if side == Side::BuyLong {
                if let Unit::Quantity(v) = stop_profit_condition {
                    if v <= price {
                        anyhow::bail!(
                            "product {}: buy long, but stop profit condition <= open price: {} <= {}",
                            product,
                            v,
                            price
                        );
                    }
                }

                if let Unit::Quantity(v) = stop_loss_condition {
                    if v >= price {
                        anyhow::bail!(
                            "product {}: buy long, but stop loss condition >= open price: {} >= {}",
                            product,
                            v,
                            price
                        );
                    }
                }
            } else {
                if let Unit::Quantity(v) = stop_profit_condition {
                    if v >= price {
                        anyhow::bail!(
                            "product {}: sell short, but stop profit condition >= open price: {} >= {}",
                            product,
                            v,
                            price
                        );
                    }
                }

                if let Unit::Quantity(v) = stop_loss_condition {
                    if v <= price {
                        anyhow::bail!(
                            "product {}: sell short, but stop loss condition <= open price: {} <= {}",
                            product,
                            v,
                            price
                        );
                    }
                }
            };

            let price = if price >= k.close {
                Price::GreaterThanMarket(price)
            } else {
                Price::LessThanMarket(price)
            };

            let ds = match (stop_profit_condition, stop_loss_condition) {
                (Unit::Quantity(a), Unit::Ignore) => DelegateState::OpenProfit(
                    Delegate {
                        side,
                        price,
                        quantity,
                        margin,
                        append_margin: 0.0,
                    },
                    Delegate {
                        side: if side == Side::BuyLong {
                            Side::BuySell
                        } else {
                            Side::SellLong
                        },
                        price: if side == Side::BuyLong {
                            match stop_profit {
                                Unit::Quantity(b) => Price::GreaterThanLimit(a, b),
                                _ => Price::GreaterThanMarket(a),
                            }
                        } else {
                            match stop_profit {
                                Unit::Quantity(b) => Price::LessThanLimit(a, b),
                                _ => Price::LessThanMarket(a),
                            }
                        },
                        quantity,
                        margin,
                        append_margin: 0.0,
                    },
                ),
                (Unit::Ignore, Unit::Quantity(a)) => DelegateState::OpenLoss(
                    Delegate {
                        side,
                        price,
                        quantity,
                        margin,
                        append_margin: 0.0,
                    },
                    Delegate {
                        side: if side == Side::BuyLong {
                            Side::BuySell
                        } else {
                            Side::SellLong
                        },
                        price: if side == Side::BuyLong {
                            match stop_loss {
                                Unit::Quantity(b) => Price::LessThanLimit(a, b),
                                _ => Price::LessThanMarket(a),
                            }
                        } else {
                            match stop_loss {
                                Unit::Quantity(b) => Price::GreaterThanLimit(a, b),
                                _ => Price::GreaterThanMarket(a),
                            }
                        },
                        quantity,
                        margin,
                        append_margin: 0.0,
                    },
                ),
                (Unit::Quantity(a), Unit::Quantity(b)) => DelegateState::OpenProfitLoss(
                    Delegate {
                        side,
                        price,
                        quantity,
                        margin,
                        append_margin: 0.0,
                    },
                    Delegate {
                        side: if side == Side::BuyLong {
                            Side::BuySell
                        } else {
                            Side::SellLong
                        },
                        price: if side == Side::BuyLong {
                            match stop_profit {
                                Unit::Quantity(v) => Price::GreaterThanLimit(a, v),
                                _ => Price::GreaterThanMarket(a),
                            }
                        } else {
                            match stop_profit {
                                Unit::Quantity(v) => Price::LessThanLimit(a, v),
                                _ => Price::LessThanMarket(a),
                            }
                        },
                        quantity,
                        margin,
                        append_margin: 0.0,
                    },
                    Delegate {
                        side: if side == Side::BuyLong {
                            Side::BuySell
                        } else {
                            Side::SellLong
                        },
                        price: if side == Side::BuyLong {
                            match stop_loss {
                                Unit::Quantity(v) => Price::LessThanLimit(b, v),
                                _ => Price::LessThanMarket(b),
                            }
                        } else {
                            match stop_loss {
                                Unit::Quantity(v) => Price::GreaterThanLimit(b, v),
                                _ => Price::GreaterThanMarket(b),
                            }
                        },
                        quantity,
                        margin,
                        append_margin: 0.0,
                    },
                ),
                _ => DelegateState::Single(Delegate {
                    side,
                    price,
                    quantity,
                    margin,
                    append_margin: 0.0,
                }),
            };

            self.balance -= margin + fee;

            self.id += 1;

            delegate.push((self.id, ds));

            return Ok(self.id);
        }

        if let Some(position) = position {
            if side == Side::BuySell && position.side == Side::SellShort {
                anyhow::bail!(
                    "product {}: buy sell, but position side is sell short",
                    product,
                );
            }

            if side == Side::SellLong && position.side == Side::BuyLong {
                anyhow::bail!(
                    "product {}: sell long, but position side is buy long",
                    product,
                );
            }

            let price = if price == 0.0 { k.close } else { price };

            // 委托数量
            let quantity = match quantity {
                Unit::Ignore => position.quantity,
                Unit::Quantity(v) => v,
                Unit::Proportion(v) => {
                    (position.quantity * v / price / *min_size).floor() as f64 * *min_size
                }
            };

            // 委托数量不能小于最小委托数量。
            if quantity < *min_size {
                anyhow::bail!(
                    "product {}: close quantity < min size: {} < {}",
                    product,
                    quantity,
                    min_size
                );
            }

            // 委托数量的价值
            let quantity_value = position.open_price * quantity;

            // 委托数量价值不能小于最小委托价值
            if quantity_value < *min_notional {
                anyhow::bail!(
                    "product {}: close quantity value < min notional : {} < {}",
                    product,
                    quantity_value,
                    min_notional
                );
            }

            // 平仓量要小于持仓量
            if quantity > position.quantity {
                anyhow::bail!(
                    "product {}: close quantity > position quantity: {} > {}",
                    product,
                    quantity,
                    position.quantity,
                );
            };

            self.id += 1;

            delegate.push((
                self.id,
                DelegateState::Single(Delegate {
                    side,
                    price: if price >= k.close {
                        Price::GreaterThanMarket(price)
                    } else {
                        Price::LessThanMarket(price)
                    },
                    quantity,
                    margin: quantity / position.quantity * position.margin,
                    append_margin: 0.0,
                }),
            ));

            return Ok(self.id);
        }

        anyhow::bail!("no position: {}", product);
    }

    /// 取消委托。
    ///
    /// * `id` 委托 id。
    pub fn cancel(&mut self, id: u64) -> bool {
        if id == 0 {
            self.product.iter_mut().for_each(|v| v.1.delegate.clear());
            return true;
        }

        for i in self.product.iter_mut() {
            if let Some(v) = i.1.delegate.iter().position(|v| v.0 == id) {
                match i.1.delegate[v].1 {
                    DelegateState::Single(v)
                        if v.side == Side::BuyLong || v.side == Side::SellShort =>
                    {
                        self.balance += v.margin
                            + match v.price {
                                Price::GreaterThanMarket(v) => v,
                                Price::LessThanMarket(v) => v,
                                Price::GreaterThanLimit(v, _) => v,
                                Price::LessThanLimit(v, _) => v,
                            } * v.quantity
                                * self.config.open_fee;
                    }
                    DelegateState::Hedging(.., v)
                    | DelegateState::HedgingProfit(_, v, ..)
                    | DelegateState::HedgingLoss(_, v, ..)
                    | DelegateState::HedgingProfitLoss(_, v, ..)
                    | DelegateState::OpenProfit(v, ..)
                    | DelegateState::OpenLoss(v, ..)
                    | DelegateState::OpenProfitLoss(v, ..) => {
                        self.balance += v.margin
                            + match v.price {
                                Price::GreaterThanMarket(v) => v,
                                Price::LessThanMarket(v) => v,
                                Price::GreaterThanLimit(v, _) => v,
                                Price::LessThanLimit(v, _) => v,
                            } * v.quantity
                                * self.config.open_fee;
                    }
                    _ => {}
                }
                i.1.delegate.remove(v);
                return true;
            }
        }

        false
    }

    /// 刷新。
    pub fn update(&mut self) {
        self.update_liquidation();
        self.update_close_delegate();
        self.update_open_delegate();
        self.update_profit_loss();
    }

    fn update_liquidation(&mut self) {
        for (.., Message { k, position, .. }) in self.product.iter_mut() {
            if position.is_none() {
                continue;
            }

            let current_position = position.as_mut().unwrap();

            if !(current_position.side == Side::BuyLong
                && k.low <= current_position.liquidation_price
                || current_position.side == Side::SellShort
                    && k.high >= current_position.liquidation_price)
            {
                continue;
            }

            let record = Record {
                side: if current_position.side == Side::BuyLong {
                    Side::BuySell
                } else {
                    Side::SellLong
                },
                price: current_position.liquidation_price,
                quantity: current_position.quantity,
                margin: current_position.margin,
                fee: 0.0,
                profit: -current_position.margin,
                profit_ratio: -1.0,
                time: k.time,
            };

            current_position.log.push(record);

            self.history
                .push(new_history_position(position.take().unwrap()));
        }
    }

    fn update_close_delegate(&mut self) {
        let mut handle =
            |k: &K, delegate_state: &mut DelegateState, position: &mut Option<Position>| {
                let mut flag = 0;

                macro_rules! remove_or_convert {
                    () => {
                        match delegate_state {
                            DelegateState::Hedging(.., v) => {
                                *delegate_state = DelegateState::Single(*v);
                                false
                            }
                            DelegateState::HedgingProfit(.., a, b) => {
                                *delegate_state = DelegateState::OpenProfit(*a, *b);
                                false
                            }
                            DelegateState::HedgingLoss(.., a, b) => {
                                *delegate_state = DelegateState::OpenLoss(*a, *b);
                                false
                            }
                            DelegateState::HedgingProfitLoss(.., a, b, c) => {
                                *delegate_state = DelegateState::OpenProfitLoss(*a, *b, *c);
                                false
                            }
                            _ => true,
                        }
                    };
                }

                loop {
                    let delegate = match delegate_state {
                        DelegateState::Single(v)
                            if v.side == Side::BuySell || v.side == Side::SellLong =>
                        {
                            v
                        }
                        DelegateState::Hedging(v, ..)
                        | DelegateState::HedgingProfit(v, ..)
                        | DelegateState::HedgingLoss(v, ..)
                        | DelegateState::HedgingProfitLoss(v, ..) => v,
                        DelegateState::ProfitLoss(a, b) => {
                            if flag == 0 {
                                flag = 1;
                                b
                            } else if flag == 1 {
                                flag = 2;
                                a
                            } else {
                                return false;
                            }
                        }
                        _ => return false,
                    };

                    let current_position = if let Some(v) = position {
                        // 如果委托方向不等于仓位方向，则撤销委托，这是由于对冲仓位导致的。
                        if delegate.side == Side::BuySell && v.side == Side::SellShort
                            || delegate.side == Side::SellLong && v.side == Side::BuyLong
                        {
                            return remove_or_convert!();
                        }

                        // 如果平仓委托的平仓量大于持仓量，则撤销委托
                        if delegate.quantity > v.quantity {
                            return remove_or_convert!();
                        }

                        v
                    } else {
                        // 如果仓位被强平，则撤销委托
                        return remove_or_convert!();
                    };

                    if !match delegate.price {
                        Price::GreaterThanMarket(v) | Price::GreaterThanLimit(v, _) => k.high >= v,
                        Price::LessThanMarket(v) | Price::LessThanLimit(v, _) => k.low <= v,
                    } {
                        if flag == 1 {
                            continue;
                        }

                        return false;
                    }

                    match delegate.price {
                        Price::GreaterThanMarket(v) | Price::LessThanMarket(v) => {
                            // 限价委托
                            let profit = if current_position.side == Side::BuyLong {
                                (v - current_position.open_price) * delegate.quantity
                            } else {
                                (current_position.open_price - v) * delegate.quantity
                            };

                            let record = Record {
                                side: delegate.side,
                                price: v,
                                quantity: delegate.quantity,
                                margin: delegate.margin + delegate.append_margin,
                                fee: v * delegate.quantity * self.config.close_fee,
                                profit,
                                profit_ratio: profit / delegate.margin,
                                time: k.time,
                            };

                            self.balance += record.profit + record.margin - record.fee;

                            current_position.quantity -= delegate.quantity;
                            current_position.margin -= delegate.margin;
                            current_position.log.push(record);

                            if current_position.quantity == 0.0 {
                                self.history
                                    .push(new_history_position(position.take().unwrap()));
                            }

                            return remove_or_convert!();
                        }
                        Price::GreaterThanLimit(a, b) | Price::LessThanLimit(a, b) => {
                            // 限价触发，限价委托
                            let temp = if delegate.side == Side::BuySell && a <= b {
                                //                   C
                                //          B        |
                                // A        |        |
                                // |        |        |
                                // open  condition  price
                                Delegate {
                                    side: delegate.side,
                                    price: Price::GreaterThanMarket(b),
                                    quantity: delegate.quantity,
                                    margin: delegate.margin,
                                    append_margin: 0.0,
                                }
                            } else if delegate.side == Side::BuySell {
                                //
                                //          B
                                // A        |        C
                                // |        |        |
                                // open  condition  price
                                Delegate {
                                    side: delegate.side,
                                    price: Price::LessThanMarket(b),
                                    quantity: delegate.quantity,
                                    margin: delegate.margin,
                                    append_margin: 0.0,
                                }
                            } else if delegate.side == Side::SellLong && a >= b {
                                // A
                                // |        B
                                // |        |        C
                                // |        |        |
                                // open  condition  price
                                Delegate {
                                    side: delegate.side,
                                    price: Price::LessThanMarket(b),
                                    quantity: delegate.quantity,
                                    margin: delegate.margin,
                                    append_margin: 0.0,
                                }
                            } else {
                                // A                 C
                                // |        B        |
                                // |        |        |
                                // |        |        |
                                // open  condition  price
                                Delegate {
                                    side: delegate.side,
                                    price: Price::GreaterThanMarket(b),
                                    quantity: delegate.quantity,
                                    margin: delegate.margin,
                                    append_margin: 0.0,
                                }
                            };

                            if flag != 0 {
                                *delegate_state = DelegateState::Single(temp);
                            } else {
                                *delegate = temp;
                            }
                        }
                    }
                }
            };

        for (
            ..,
            Message {
                k,
                delegate,
                position,
                ..
            },
        ) in self.product.iter_mut()
        {
            let mut i = 0;

            while i < delegate.len() {
                if handle(k, &mut delegate[i].1, position) {
                    delegate.remove(i);
                } else {
                    i += 1;
                }
            }
        }
    }

    fn update_open_delegate(&mut self) {
        enum State {
            Next,
            Close(DelegateState),
            ReloadRemove,
            Remove,
        }

        let handle = |product: &String,
                      k: &K,
                      delegate_state: &mut DelegateState,
                      position: &mut Option<Position>| {
            let delegate = match delegate_state {
                DelegateState::Single(v)
                    if v.side == Side::BuyLong || v.side == Side::SellShort =>
                {
                    v
                }
                DelegateState::OpenProfit(v, ..)
                | DelegateState::OpenLoss(v, ..)
                | DelegateState::OpenProfitLoss(v, ..) => v,
                _ => return State::Next,
            };

            if !match delegate.price {
                Price::GreaterThanMarket(v) | Price::GreaterThanLimit(v, _) => k.high >= v,
                Price::LessThanMarket(v) | Price::LessThanLimit(v, _) => k.low <= v,
            } {
                return State::Next;
            }

            let price = match delegate.price {
                Price::GreaterThanMarket(v) => v,
                Price::LessThanMarket(v) => v,
                Price::GreaterThanLimit(v, _) => v,
                Price::LessThanLimit(v, _) => v,
            };

            // 计算开仓均价
            // 新方向，新价格，新持仓量，新保证金，追加保证金
            let (new_side, new_price, new_quantity, new_margin, append_margin) = match position {
                Some(v) => {
                    if v.side == delegate.side {
                        // 加仓
                        let quantity = v.quantity + delegate.quantity;

                        let open_price = ((v.open_price * v.quantity)
                            + (price * delegate.quantity))
                            / (v.quantity + delegate.quantity);

                        let append_margin = (v.margin
                            - v.open_price * v.quantity / self.config.lever as f64)
                            + (delegate.margin
                                - open_price * delegate.quantity / self.config.lever as f64);

                        (
                            delegate.side,
                            open_price,
                            quantity,
                            v.margin + delegate.margin,
                            append_margin,
                        )
                    } else {
                        // 虽然在委托的时候会处理减仓，但是要存在仓位的时候才会减仓
                        // 这里处理多个委托同时成交，且方向不同的情况
                        return if v.quantity < delegate.quantity {
                            let new_margin = v.quantity / delegate.quantity * delegate.margin;
                            let sub_margin = delegate.margin - new_margin;
                            delegate.quantity = delegate.quantity - v.quantity;
                            delegate.margin = new_margin;
                            State::Close(DelegateState::Single(Delegate {
                                side: if v.side == Side::BuyLong {
                                    Side::BuySell
                                } else {
                                    Side::SellLong
                                },
                                price: delegate.price,
                                quantity: v.quantity,
                                margin: v.margin,
                                append_margin: sub_margin,
                            }))
                        } else {
                            delegate.side = if v.side == Side::BuyLong {
                                Side::BuySell
                            } else {
                                Side::SellLong
                            };
                            delegate.append_margin = delegate.margin;
                            delegate.margin = delegate.quantity / v.quantity * v.margin;
                            *delegate_state = DelegateState::Single(*delegate);
                            State::ReloadRemove
                        };
                    }
                }
                _ => (
                    delegate.side,
                    price,
                    delegate.quantity,
                    delegate.margin,
                    delegate.margin - price * delegate.quantity / self.config.lever as f64,
                ),
            };

            // 计算吃单手续费是为了防止穿仓，即余额不够支付手续费的情况
            // 做多强平价格 = 入场价格 × (1 - 初始保证金率 + 维持保证金率) - (追加保证金 / 仓位数量) + 吃单手续费
            // 做空强平价格 = 入场价格 × (1 + 初始保证金率 - 维持保证金率) + (追加保证金 / 仓位数量) - 吃单手续费
            // 初始保证金率 = 1 / 杠杆
            // 追加保证金 = 账户余额 - 初始化保证金
            // 初始保证金 = 入场价格 / 杠杆
            let imr = 1.0 / self.config.lever as f64;
            let mmr = self.config.maintenance;
            let mut liquidation_price = if new_side == Side::BuyLong {
                new_price * (1.0 - imr + mmr) - (append_margin / new_quantity)
                    + price * delegate.quantity * self.config.close_fee
            } else {
                new_price * (1.0 + imr - mmr) + (append_margin / new_quantity)
                    - price * delegate.quantity * self.config.close_fee
            };

            if liquidation_price < 0.0 {
                liquidation_price = 0.0;
            }

            let price = match delegate.price {
                Price::GreaterThanMarket(v) => v,
                Price::LessThanMarket(v) => v,
                Price::GreaterThanLimit(v, _) => v,
                Price::LessThanLimit(v, _) => v,
            };

            // 交易记录
            let record = Record {
                side: delegate.side,
                price,
                quantity: delegate.quantity,
                margin: delegate.margin,
                fee: price * delegate.quantity * self.config.open_fee,
                profit: 0.0,
                profit_ratio: 0.0,
                time: k.time,
            };

            match position {
                Some(v) => {
                    // 如果已经存在仓位，则直接修改仓位
                    v.side = new_side;
                    v.open_price = new_price;
                    v.quantity = new_quantity;
                    v.margin = new_margin;
                    v.liquidation_price = liquidation_price;
                    v.log.push(record);
                }
                None => {
                    // 新建仓位
                    let mut current_position = Position {
                        product: product.clone(),
                        lever: self.config.lever,
                        side: new_side,
                        open_price: new_price,
                        quantity: new_quantity,
                        margin: new_margin,
                        liquidation_price,
                        close_price: 0.0,
                        profit: 0.0,
                        profit_ratio: 0.0,
                        fee: 0.0,
                        open_time: k.time,
                        close_time: 0,
                        log: Vec::new(),
                    };

                    current_position.log.push(record);

                    position.replace(current_position);
                }
            };

            match delegate_state {
                DelegateState::OpenProfit(.., v) => {
                    *delegate_state = DelegateState::Single(*v);
                    State::Next
                }
                DelegateState::OpenLoss(.., v) => {
                    *delegate_state = DelegateState::Single(*v);
                    State::Next
                }
                DelegateState::OpenProfitLoss(.., a, b) => {
                    *delegate_state = DelegateState::ProfitLoss(*a, *b);
                    State::Next
                }
                _ => State::Remove,
            }
        };

        for (
            product,
            Message {
                k,
                delegate,
                position,
                ..
            },
        ) in self.product.iter_mut()
        {
            let mut i = 0;

            while i < delegate.len() {
                match handle(product, k, &mut delegate[i].1, position) {
                    State::Next => {
                        i += 1;
                    }
                    State::Close(v) => {
                        delegate.insert(0, (0, v));
                        self.update_close_delegate();
                        self.update_open_delegate();
                        return;
                    }
                    State::ReloadRemove => {
                        self.update_close_delegate();
                        self.update_open_delegate();
                        return;
                    }
                    State::Remove => {
                        delegate.remove(i);
                    }
                }
            }
        }
    }

    fn update_profit_loss(&mut self) {
        for (.., Message { k, position, .. }) in self.product.iter_mut() {
            if let Some(v) = position {
                let profit = if v.side == Side::BuyLong {
                    (k.close - v.open_price) * v.quantity
                } else {
                    (v.open_price - k.close) * v.quantity
                };
                v.profit = profit;
                v.profit_ratio = profit / v.margin
            }
        }
    }
}

/// 根据 log 统计仓位。
///
/// * `最大持仓量`。
/// * `最大保证金`。
/// * `收益`。
/// * `收益率`。
/// * `手续费`。
/// * `最后平仓价格`。
/// * `最后平仓时间`。
fn new_history_position(mut position: Position) -> Position {
    position.profit = position.log.iter().map(|v| v.profit).sum();
    position.profit_ratio = position.log.iter().map(|v| v.profit_ratio).sum();
    position.fee = position.log.iter().map(|v| v.fee).sum();
    let mut max_quantity = 0.0;
    let mut sum_quantity = 0.0;
    let mut max_margin = 0.0;
    let mut sum_margin = 0.0;

    position.log.iter().for_each(|v| {
        sum_quantity += if v.side == Side::BuyLong || v.side == Side::SellShort {
            v.quantity
        } else {
            -v.quantity
        };

        if sum_quantity > max_quantity {
            max_quantity = sum_quantity;
        }

        sum_margin += if v.side == Side::BuyLong || v.side == Side::SellShort {
            v.margin
        } else {
            -v.margin
        };

        if sum_margin > max_margin {
            max_margin = sum_margin;
        }
    });

    position.quantity = max_quantity;
    position.margin = max_margin;
    position.close_price = position.log.last().unwrap().price;
    position.close_time = position.log.last().unwrap().time;
    position
}
