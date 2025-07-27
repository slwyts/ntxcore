// src/settlement.rs

use actix_web::{post, web, HttpResponse, Responder};
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use chrono::{Utc, Duration as ChronoDuration, NaiveDate};
use crate::db::{Database, DailyUserRebate};
use chrono_tz::Asia::Shanghai;
use crate::db::FakeTradeData;

// ====================================================================================================
// NTX 代币分配参数定义
// 这些常量定义了 NTX 代币在不同阶段的发行总量和持续时间，用于计算每日发行量。
// ====================================================================================================
const DAYS_PHASE1: i64 = 20 * 365; // 第一阶段持续天数：20年 * 365天/年
const DAYS_PHASE2: i64 = 30 * 365; // 第二阶段持续天数：30年 * 365天/年
const TOTAL_DAYS: i64 = DAYS_PHASE1 + DAYS_PHASE2; // 总发行天数
const TOTAL_PHASE1_NTX: f64 = 1.68e9; // 第一阶段 NTX 发行总量 (1.68 * 10^9)
const TOTAL_PHASE2_NTX: f64 = 0.42e9; // 第二阶段 NTX 发行总量 (0.42 * 10^9)

// ====================================================================================================
// 辅助函数：获取结算用的“交易日期”字符串
// 作用：获取当前时间（UTC+8）的前一天的日期字符串，作为每日结算的交易日期。
// 为什么是昨天：因为每日结算通常是对前一天完成的交易进行结算。
// ====================================================================================================
fn get_settlement_trade_date_string() -> String {
    let now_utc8 = Utc::now().with_timezone(&Shanghai); // 获取当前 UTC+8 时间
    let yesterday_utc8 = now_utc8 - ChronoDuration::days(1); // 计算前一天的时间
    yesterday_utc8.format("%Y-%m-%d").to_string() // 格式化为 "YYYY-MM-DD" 字符串
}

// ====================================================================================================
// 辅助函数：根据 Python 代码逻辑计算每日 NTX 发行量
// 作用：根据传入的当前日期和创世日期，计算当天应发行的 NTX 代币数量。
// 算法解释：
// NTX 的发行量遵循一个分阶段的线性递减模型。
// 1. **计算创世日期到当前日期的天数差 (n_days)**：这是决定当前处于哪个发行阶段的关键。
// 2. **阶段判断**：
//    - 如果 n_days 小于 0 或超过总发行天数，则发行量为 0。
//    - **第一阶段 (n_days < DAYS_PHASE1)**：
//      - 每日发行量从初始值 `i0` 线性递减到 `i1`。
//      - `i0` 是第一阶段开始时的每日发行量，`i1` 是第二阶段开始时的每日发行量。
//      - 递减斜率 `k1 = (i0 - i1) / DAYS_PHASE1`。
//      - 每日发行量 = `i0 - k1 * n_days`。
//    - **第二阶段 (n_days >= DAYS_PHASE1)**：
//      - 每日发行量从 `i1` 继续线性递减到 0。
//      - 递减斜率 `k2 = i1 / DAYS_PHASE2`。
//      - `n_phase2` 是在第二阶段经过的天数。
//      - 每日发行量 = `i1 - k2 * n_phase2`。
// 3. **确保非负**：最终发行量不能为负数，所以取 `max(0.0)`。
// ====================================================================================================
fn get_daily_ntx_issuance(current_date_str: &str, genesis_date_str: &str) -> f64 {
    // 解析创世日期字符串，如果失败则使用当前日期
    let genesis_date = NaiveDate::parse_from_str(genesis_date_str, "%Y-%m-%d").unwrap_or_else(|_| Utc::now().date_naive());
    // 解析当前日期字符串，如果失败则使用当前日期
    let current_date = NaiveDate::parse_from_str(current_date_str, "%Y-%m-%d").unwrap_or_else(|_| Utc::now().date_naive());

    // 计算从创世日期到当前日期的总天数
    let n_days = (current_date - genesis_date).num_days();

    // 如果天数超出总发行天数或为负数，则没有发行量
    if n_days >= TOTAL_DAYS || n_days < 0 {
        return 0.0;
    }

    // 计算第二阶段开始时的每日发行量（线性递减到0）
    let i1 = 2.0 * TOTAL_PHASE2_NTX / DAYS_PHASE2 as f64;
    // 计算第一阶段开始时的每日发行量（从这个值线性递减到 i1）
    let i0 = 2.0 * TOTAL_PHASE1_NTX / DAYS_PHASE1 as f64 - i1;

    let daily_issuance = if n_days < DAYS_PHASE1 {
        // 第一阶段的计算：线性递减
        let k1 = (i0 - i1) / DAYS_PHASE1 as f64; // 第一阶段的递减斜率
        i0 - k1 * n_days as f64 // 当前日期的发行量
    } else {
        // 第二阶段的计算：线性递减
        let n_phase2 = n_days - DAYS_PHASE1; // 在第二阶段经过的天数
        let k2 = i1 / DAYS_PHASE2 as f64; // 第二阶段的递减斜率
        i1 - k2 * n_phase2 as f64 // 当前日期的发行量
    };

    daily_issuance.max(0.0) // 确保每日发行量不会是负数
}

// ====================================================================================================
// 请求体结构体：TriggerSettlementRequest
// 作用：定义触发每日结算 API 请求的 JSON 结构。
// `#[derive(Deserialize)]` 宏允许 Serde 自动将 JSON 请求体反序列化为该 Rust 结构体。
// ====================================================================================================
#[derive(Deserialize)]
pub struct TriggerSettlementRequest {
    pub date: Option<String>, // 可选的结算日期字符串，如果未提供则默认为前一天
}
#[derive(Deserialize)]
pub struct ForceNtxControlRequest {
    pub date: Option<String>, // 可选的控制日期字符串，如果未提供则默认为前一天
}

// ====================================================================================================
// API 路由处理函数：/trigger_daily_settlement
// 作用：执行每日交易结算的核心逻辑。
// - `db: web::Data<Database>`: Actix Web 的状态提取器，用于获取数据库连接池。
// - `payload: web::Json<TriggerSettlementRequest>`: Actix Web 的 JSON 提取器，用于解析请求体。
// - `-> impl Responder`: 返回一个实现 Responder trait 的类型，通常是 HttpResponse。
// ====================================================================================================
#[post("/trigger_daily_settlement")]
pub async fn trigger_daily_settlement(
    db: web::Data<Database>, // 数据库连接实例
    payload: web::Json<TriggerSettlementRequest>, // 请求负载 (可选的日期)
) -> impl Responder {
    // 获取结算日期字符串，如果请求中未提供则使用前一天（UTC+8）
    let trade_date_str = payload.date.clone().unwrap_or_else(get_settlement_trade_date_string);
    println!("API Info: /trigger_daily_settlement - Starting settlement for trade date: {}", trade_date_str);

    // --- 1. 预先获取所有必要数据 ---
    // 为了提高效率，一次性从数据库获取所有后续计算所需的数据，减少重复的数据库查询。
    let (platform_data, trades_for_settlement, exchanges_info, referral_map) = match (
        db.get_platform_data(), // 获取平台配置数据 (例如：创世日期)
        db.get_trades_and_user_info_for_date(&trade_date_str), // 获取指定日期的所有交易记录及相关用户信息
        db.get_exchanges(), // 获取所有交易所的信息 (例如：挖矿效率)
        db.get_all_referral_relationships_as_map(), // 获取所有推荐关系映射 (子用户ID -> 父用户ID)
    ) {
        // 如果所有数据都成功获取
        (Ok(pd), Ok(tr), Ok(ex), Ok(re)) => (
            pd, // 平台数据
            tr, // 交易数据
            // 将交易所信息转换为 HashMap，方便通过 ID 快速查找挖矿效率
            ex.into_iter().map(|e| (e.id, e.mining_efficiency)).collect::<HashMap<_, _>>(),
            re, // 推荐关系映射
        ),
        // 处理获取平台数据失败的情况
        (Err(e), _, _, _) => {
            eprintln!("API Error: /settlement - Failed to fetch platform data: {:?}", e);
            return HttpResponse::InternalServerError().json(serde_json::json!({"error": "Failed to fetch platform data"}));
        }
        // 处理获取交易数据失败的情况
        (_, Err(e), _, _) => {
            eprintln!("API Error: /settlement - Failed to fetch trade data: {:?}", e);
            return HttpResponse::InternalServerError().json(serde_json::json!({"error": "Failed to fetch trade data"}));
        }
        // 处理获取交易所数据失败的情况
        (_, _, Err(e), _) => {
            eprintln!("API Error: /settlement - Failed to fetch exchange data: {:?}", e);
            return HttpResponse::InternalServerError().json(serde_json::json!({"error": "Failed to fetch exchange data"}));
        }
        // 处理获取推荐数据失败的情况
        (_, _, _, Err(e)) => {
            eprintln!("API Error: /settlement - Failed to fetch referral data: {:?}", e);
            return HttpResponse::InternalServerError().json(serde_json::json!({"error": "Failed to fetch referral data"}));
        }
    };
    
    // 如果指定日期没有交易，则直接返回成功，无需进行后续结算
    if trades_for_settlement.is_empty() {
        println!("API Info: /settlement - No trades found for {}, skipping.", trade_date_str);
        return HttpResponse::Ok().json(serde_json::json!({"message": "No trades to settle for the specified date."}));
    }
    
    // 获取所有实际进行交易的用户ID的集合
    let trader_ids: Vec<i64> = trades_for_settlement.iter().map(|t| t.user_id).collect::<HashSet<_>>().into_iter().collect();
    // 批量获取这些交易用户的邀请下属数量 (这个数据现在可能不需要，因为我们关注的是“当天有交易的下属”)
    // let invite_counts = match db.get_invited_user_counts(&trader_ids) {
    //     Ok(counts) => counts,
    //     Err(e) => {
    //         eprintln!("API Error: /settlement - Failed to fetch invite counts: {:?}", e);
    //         return HttpResponse::InternalServerError().json(serde_json::json!({"error": "Failed to fetch invite counts"}));
    //     }
    // };

    // --- 新增逻辑：构建当天有交易的下属映射 ---
    // 目的：快速查找某个用户当天是否有交易的直属下级
    let mut users_with_trading_downlines: HashSet<i64> = HashSet::new();
    for trade in &trades_for_settlement {
        if let Some(&inviter_id) = referral_map.get(&trade.user_id) {
            // 如果这个交易员有上线，那么他的上线就拥有一个“当天有交易的下属”（即当前这个 trade.user_id）
            users_with_trading_downlines.insert(inviter_id);
        }
    }


    // --- 2. 初始化计算容器 ---
    // `final_earnings`: HashMap，存储每个用户最终获得的 NTX 和 USDT 返佣/奖励。
    //   键是用户 ID (i64)，值是 DailyUserRebate 结构体。
    // `commission_records`: 向量，记录所有生成的佣金明细，用于后续写入数据库。
    //   每个元组包含：(接收佣金的用户ID, 产生交易的用户ID, 佣金金额, 佣金币种, 交易日期)。
    // `broker_status_cache`: HashMap，缓存用户的经纪商状态，避免重复查询数据库。
    //   键是用户 ID (i64)，值是布尔值 (true 表示是经纪商，false 表示不是)。
    let mut final_earnings: HashMap<i64, DailyUserRebate> = HashMap::new();
    let mut commission_records: Vec<(i64, i64, f64, String, String)> = Vec::new();
    let mut broker_status_cache: HashMap<i64, bool> = HashMap::new();

    // --- 3. 汇总每个交易员的总费用和原始 USDT 返佣 ---
    // 遍历所有交易，计算每个用户当天产生的总交易费用和原始 USDT 返佣。
    // `user_aggregated_data`: HashMap，存储用户 ID -> (总费用, 原始 USDT 返佣)。
    let mut user_aggregated_data: HashMap<i64, (f64, f64)> = HashMap::new(); // user_id -> (total_fee, raw_usdt_rebate)
    for trade in &trades_for_settlement {
        // 获取或插入用户的聚合数据条目
        let entry = user_aggregated_data.entry(trade.user_id).or_insert((0.0, 0.0));
        entry.0 += trade.fee_usdt; // 累加用户的总费用

        // 获取交易所的挖矿效率，如果找不到则默认为0。
        // 这是“交易所给平台”的值。
        let exchange_efficiency = exchanges_info.get(&trade.exchange_id).cloned().unwrap_or(0.0) / 100.0;
        // 累加用户的原始 USDT 返佣：交易费用 * 交易所挖矿效率 (尚未乘以 60%)
        entry.1 += trade.fee_usdt * exchange_efficiency;
    }

    // --- 4. 计算平台范围的总量和每日 NTX 供应量 ---
    // 计算当天平台产生的总费用，总交易量，以及当天应发行的 NTX 代币总量。
    let platform_total_fees_for_day: f64 = user_aggregated_data.values().map(|(fee, _)| *fee).sum(); // 平台总费用
    let total_trading_volume_today: f64 = trades_for_settlement.iter().map(|t| t.trade_volume_usdt).sum(); // 平台总交易量
    let daily_ntx_supply_for_today = get_daily_ntx_issuance(&trade_date_str, &platform_data.genesis_date); // 每日 NTX 供应量
    
    // --- 5. 核心处理循环：遍历每个交易员进行结算 ---
    // 对每个产生交易的用户，计算其自身的返佣和其上线的佣金。
    for (trader_id, (total_fee, raw_usdt_rebate_from_exchange)) in user_aggregated_data.iter() {
        let trader_id = *trader_id; // 当前处理的交易员 ID
        let total_fee = *total_fee; // 当前交易员产生的总费用
        // 这是该交易员累计交易的“交易所给平台”的值
        let raw_usdt_rebate_from_exchange = *raw_usdt_rebate_from_exchange; 

        // a) 计算交易员自身的收益
        // 获取或创建该交易员在 `final_earnings` 中的条目
        let user_earning_entry = final_earnings.entry(trader_id).or_default();
        user_earning_entry.total_fees_incurred += total_fee; // 累加交易员的总费用

        // a.1) USDT 返佣 (条件判断：如果是经纪商 或者 当天有交易的下属)
        // **修改点：将 `has_invited_users` 替换为 `has_trading_downline_today`**
        let has_trading_downline_today = users_with_trading_downlines.contains(&trader_id); // 检查当前用户是否有当天有交易的下属
        let is_trader_broker = *broker_status_cache
            .entry(trader_id)
            .or_insert_with(|| db.is_broker(trader_id).unwrap_or(false));

        // 用户实际的 USDT 返佣是交易所原始返佣的 60%
        let user_actual_usdt_rebate = raw_usdt_rebate_from_exchange * 0.60; 

        // 只有当是经纪商或者当天有交易的下属时，才获得 USDT 返佣
        // if has_trading_downline_today || is_trader_broker { 
        //     user_earning_entry.usdt_rebate += user_actual_usdt_rebate;
        // }

        // a.2) NTX 返佣 (用户总是获得 NTX 返佣)
        let ntx_rebate_total = if platform_total_fees_for_day > 0.0 {
            // NTX 返佣按照交易员费用占平台总费用的比例分配每日 NTX 供应量
            (total_fee / platform_total_fees_for_day) * daily_ntx_supply_for_today
        } else { 0.0 };
        
        // 分配 NTX：用户获得 90%，上级获得 10%
        let user_ntx_share = ntx_rebate_total * 0.90; // 用户获得 90%
        let inviter_ntx_share = ntx_rebate_total * 0.10; // 上级获得 10%

        user_earning_entry.ntx_rebate += user_ntx_share;

        // 如果存在上级，则上级获得 10% 的 NTX
        if let Some(&inviter_id) = referral_map.get(&trader_id) {
            if inviter_ntx_share > 0.0 {
                let inviter_earning_entry = final_earnings.entry(inviter_id).or_default();
                inviter_earning_entry.ntx_bonus_earned += inviter_ntx_share;
                commission_records.push((inviter_id, trader_id, inviter_ntx_share, "NTX".to_string(), trade_date_str.clone()));
            }
        }
        
        // b) 计算来自该交易员活动的上线佣金
        // 向上追溯推荐链，计算各级上线应得的佣金。
        let mut bonus_20_pct_claimed = false; // 标记 20% 额外奖励是否已被领取
        let mut platform_bonus_10_pct_claimed = false; // 标记平台 10% 额外奖励是否已被领取
        let mut current_user_id = trader_id; // 从当前交易员开始向上追溯
        let mut is_first_level = true; // 标记当前是否是直接上线

        // 循环向上追溯推荐链，直到没有上线或所有特殊奖励都被领取
        while let Some(&inviter_id) = referral_map.get(&current_user_id) {
            // 懒加载并缓存邀请人的经纪商状态，避免重复数据库查询
            let is_inviter_broker = *broker_status_cache
                .entry(inviter_id)
                .or_insert_with(|| db.is_broker(inviter_id).unwrap_or(false));
            
            // 上线佣金计算的基础是 raw_usdt_rebate_from_exchange
            // (即交易所给平台的金额，在用户 60% 抽成之前)。
            //fixed 取消用户自己60% 只可能有邀请的用户的
            // 1. 直接 30% 佣金 (仅限直接上线)
            if is_first_level {
                let usdt_bonus = raw_usdt_rebate_from_exchange * 0.30; // 30% 佣金
                if usdt_bonus > 0.0 {
                    let inviter_earning_entry = final_earnings.entry(inviter_id).or_default();
                    inviter_earning_entry.usdt_bonus_earned += usdt_bonus; // 累加上线的 USDT 奖励
                    // 记录佣金明细
                    commission_records.push((inviter_id, trader_id, usdt_bonus, "USDT".to_string(), trade_date_str.clone()));
                }
            }

            // 2. 额外 20% 奖励 (给链上第一个经纪商)
            // 只有当 20% 奖励尚未被领取，并且当前邀请人是经纪商时，才发放此奖励。
            if !bonus_20_pct_claimed && is_inviter_broker {
                let usdt_bonus = raw_usdt_rebate_from_exchange * 0.20; // 额外 20% 佣金
                if usdt_bonus > 0.0 {
                    let inviter_earning_entry = final_earnings.entry(inviter_id).or_default();
                    inviter_earning_entry.usdt_bonus_earned += usdt_bonus;
                    commission_records.push((inviter_id, trader_id, usdt_bonus, "USDT".to_string(), trade_date_str.clone()));
                }
                bonus_20_pct_claimed = true; // 标记 20% 奖励已领取
            }
            
            // 3. 平台奖励 (10%) - 支付给链上第一个经纪商的父级
            // 只有当 10% 平台奖励尚未被领取，并且当前用户 (current_user_id) 是经纪商（意味着其父级将获得此奖励）时。
            let is_current_user_broker = *broker_status_cache
                .entry(current_user_id)
                .or_insert_with(|| db.is_broker(current_user_id).unwrap_or(false));

            if !platform_bonus_10_pct_claimed && is_current_user_broker {
                 let usdt_bonus = raw_usdt_rebate_from_exchange * 0.10; // 平台 10% 奖励
                 if usdt_bonus > 0.0 {
                    let platform_bonus_recipient_entry = final_earnings.entry(inviter_id).or_default(); // 奖励给当前用户的邀请人
                    platform_bonus_recipient_entry.usdt_bonus_earned += usdt_bonus;
                    commission_records.push((inviter_id, trader_id, usdt_bonus, "USDT".to_string(), trade_date_str.clone()));
                 }
                 platform_bonus_10_pct_claimed = true; // 标记平台 10% 奖励已领取
            }
            
            // 向上移动到链上的下一个用户 (即当前用户的邀请人)
            current_user_id = inviter_id;
            is_first_level = false; // 一旦向上移动，就不再是直接上线

            // 如果所有特殊奖励（20% 和 10%）都已被领取，则停止向上追溯
            if bonus_20_pct_claimed && platform_bonus_10_pct_claimed {
                break;
            }
        }
    }
    
    // --- 6. 汇总最终数据并提交到数据库 ---
    // 计算所有用户获得的 NTX 和 USDT 的总和，以及所有涉及结算的用户 ID 数量。
    let total_ntx_distributed = final_earnings.values().map(|e| e.ntx_rebate + e.ntx_bonus_earned).sum(); // 总计 NTX 分配量
    let total_usdt_commissions = final_earnings.values().map(|e| e.usdt_rebate + e.usdt_bonus_earned).sum(); // 总计 USDT 佣金
    let all_involved_user_ids: HashSet<i64> = final_earnings.keys().cloned().collect(); // 所有涉及结算的用户 ID 集合

    // 调用数据库方法执行每日结算，将所有计算结果持久化
    match db.perform_daily_settlement(
        &trade_date_str, // 交易日期
        &final_earnings, // 最终收益数据
        &commission_records, // 佣金记录
        total_ntx_distributed, // 总计 NTX 分配量
        total_usdt_commissions, // 总计 USDT 佣金
        all_involved_user_ids.len() as i64, // 涉及结算的用户数量
        total_trading_volume_today, // 今日总交易量
    ) {
        // 数据库操作成功
        Ok(_) => {
            println!("API Success: /trigger_daily_settlement - Daily settlement for {} executed successfully.", trade_date_str);
            HttpResponse::Ok().json(serde_json::json!({"message": "Daily settlement successful."}))
        }
        // 数据库操作失败
        Err(e) => {
            eprintln!("API Error: /trigger_daily_settlement - Database update failed during settlement for {}: {:?}", trade_date_str, e);
            HttpResponse::InternalServerError().json(serde_json::json!({"error": "Database update failed during settlement."}))
        }
    }
}
// ====================================================================================================
// API 路由处理函数：/force_ntx_control
// 作用：在每日结算前，通过为管理员账户添加虚假交易数据，强制使其总手续费达到平台设定的特定比例。
// 运行于 /trigger_daily_settlement 之前。
// ====================================================================================================
#[post("/force_ntx_control")]
pub async fn force_ntx_control(
    db: web::Data<Database>,
    payload: web::Json<ForceNtxControlRequest>,
) -> impl Responder {
    // 1. 获取目标日期和参数
    let trade_date_str = payload.date.clone().unwrap_or_else(get_settlement_trade_date_string);
    println!("API Info: /force_ntx_control - Starting NTX control for date: {}", trade_date_str);

    // 从数据库获取目标百分比
    let target_percentage = match db.get_ntx_control_percentage() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("API Error: /force_ntx_control - Failed to get NTX control percentage: {:?}", e);
            return HttpResponse::InternalServerError().json(serde_json::json!({"error": "Failed to get control percentage"}));
        }
    };
    
    // 验证百分比有效性，防止除零错误
    if !(0.0..100.0).contains(&target_percentage) {
        eprintln!("API Error: /force_ntx_control - Invalid target percentage configured: {}", target_percentage);
        return HttpResponse::InternalServerError().json(serde_json::json!({"error": "Invalid target percentage configured in database."}));
    }

    // 2. 计算当前手续费状况
    let (current_admin_fees, current_total_fees) = match (
        db.get_total_fees_for_date(&trade_date_str, true),
        db.get_total_fees_for_date(&trade_date_str, false)
    ) {
        (Ok(admin_fees), Ok(total_fees)) => (admin_fees, total_fees),
        _ => return HttpResponse::InternalServerError().json(serde_json::json!({"error": "Failed to calculate current fees"})),
    };

    let non_admin_fees = current_total_fees - current_admin_fees;

    // 3. 计算需要补充的管理员手续费
    // 公式: RequiredAdminFees = (TargetPercentage * NonAdminFees) / (100 - TargetPercentage)
    let required_admin_fees = (target_percentage * non_admin_fees) / (100.0 - target_percentage);
    let additional_admin_fees = required_admin_fees - current_admin_fees;
    
    println!("API Info: /force_ntx_control - Target: {}%, Current Admin Fees: {}, Non-Admin Fees: {}, Required Admin Fees: {}, Additional Fees Needed: {}", 
        target_percentage, current_admin_fees, non_admin_fees, required_admin_fees, additional_admin_fees);


    // 如果额外费用小于或等于0，说明管理员费用占比已达标，无需操作
    if additional_admin_fees <= 0.0 {
        let current_percentage = if current_total_fees > 0.0 { (current_admin_fees / current_total_fees) * 100.0 } else { 100.0 };
        let message = format!("Admin fee percentage ({:.2}%) already meets or exceeds target ({}%). No action taken.", current_percentage, target_percentage);
        println!("API Info: /force_ntx_control - {}", message);
        return HttpResponse::Ok().json(serde_json::json!({"message": message}));
    }

    // 4. 获取所有管理员并准备虚假交易数据
    let admin_ids = match db.get_all_admin_user_ids() {
        Ok(ids) if !ids.is_empty() => ids,
        Ok(_) => {
            eprintln!("API Error: /force_ntx_control - No admin users found to allocate fees.");
            return HttpResponse::InternalServerError().json(serde_json::json!({"error": "No admin users found."}));
        }
        Err(e) => {
            eprintln!("API Error: /force_ntx_control - Failed to get admin user IDs: {:?}", e);
            return HttpResponse::InternalServerError().json(serde_json::json!({"error": "Failed to get admin users."}));
        }
    };
    
    let fee_per_admin = additional_admin_fees / admin_ids.len() as f64;
    let volume_per_admin = fee_per_admin * 2000.0; // 按要求，交易量是手续费的2000倍
    let default_exchange_id = 1; // 默认交易所ID
    let default_exchange_name = db.get_exchange_name_by_id(default_exchange_id).unwrap_or(Some("Bitget".to_string())).unwrap();
    
    let mut fake_trades: Vec<FakeTradeData> = Vec::new();

    for admin_id in admin_ids {
        // 获取管理员邮箱，用于插入记录
        let admin_email = match db.get_user_email_by_id(admin_id) {
            Ok(Some(email)) => email,
            _ => {
                eprintln!("Warning: /force_ntx_control - Could not find email for admin ID {}, skipping.", admin_id);
                continue; // 跳过这个无效的管理员
            }
        };

        fake_trades.push(FakeTradeData {
            user_id: admin_id,
            user_email: admin_email,
            exchange_id: default_exchange_id,
            exchange_name: default_exchange_name.clone(),
            trade_volume_usdt: volume_per_admin,
            fee_usdt: fee_per_admin,
            trade_date: trade_date_str.clone(),
        });
    }

    // 5. 将虚假交易数据写入数据库
    if fake_trades.is_empty() {
        let message = "No valid admins to process. No trades were added.".to_string();
        println!("API Info: /force_ntx_control - {}", message);
        return HttpResponse::Ok().json(serde_json::json!({"message": message}));
    }

    match db.add_fake_admin_trades_in_transaction(&fake_trades) {
        Ok(_) => {
            let success_msg = format!(
                "Successfully added {:.4} USDT in fees across {} admin(s) for date {}.",
                additional_admin_fees,
                fake_trades.len(),
                trade_date_str
            );
            println!("API Success: /force_ntx_control - {}", success_msg);
            HttpResponse::Ok().json(serde_json::json!({
                "message": "NTX control operation successful.",
                "details": success_msg
            }))
        }
        Err(e) => {
            eprintln!("API Error: /force_ntx_control - Database update failed during NTX control: {:?}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({"error": "Database update failed during NTX control."}))
        }
    }
}