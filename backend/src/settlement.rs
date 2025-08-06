// src/settlement.rs

use actix_web::{post, web, HttpResponse, Responder};
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use chrono::{Utc, Duration as ChronoDuration, NaiveDate};
use chrono_tz::Asia::Shanghai;
use crate::db::{Database, DailyUserRebate};
use crate::db::FakeTradeData;

// ====================================================================================================
// NTX 代币分配参数定义
// ====================================================================================================
const DAYS_PHASE1: i64 = 20 * 365;
const DAYS_PHASE2: i64 = 30 * 365;
const TOTAL_DAYS: i64 = DAYS_PHASE1 + DAYS_PHASE2;
const TOTAL_PHASE1_NTX: f64 = 1.68e9;
const TOTAL_PHASE2_NTX: f64 = 0.42e9;

// ====================================================================================================
// 辅助函数
// ====================================================================================================
fn get_settlement_trade_date_string() -> String {
    let now_utc8 = Utc::now().with_timezone(&Shanghai);
    let yesterday_utc8 = now_utc8 - ChronoDuration::days(1);
    yesterday_utc8.format("%Y-%m-%d").to_string()
}

fn get_daily_ntx_issuance(current_date_str: &str, genesis_date_str: &str) -> f64 {
    let genesis_date = NaiveDate::parse_from_str(genesis_date_str, "%Y-%m-%d").unwrap_or_else(|_| Utc::now().date_naive());
    let current_date = NaiveDate::parse_from_str(current_date_str, "%Y-%m-%d").unwrap_or_else(|_| Utc::now().date_naive());
    let n_days = (current_date - genesis_date).num_days();
    if n_days >= TOTAL_DAYS || n_days < 0 {
        return 0.0;
    }
    let i1 = 2.0 * TOTAL_PHASE2_NTX / DAYS_PHASE2 as f64;
    let i0 = 2.0 * TOTAL_PHASE1_NTX / DAYS_PHASE1 as f64 - i1;
    let daily_issuance = if n_days < DAYS_PHASE1 {
        let k1 = (i0 - i1) / DAYS_PHASE1 as f64;
        i0 - k1 * n_days as f64
    } else {
        let n_phase2 = n_days - DAYS_PHASE1;
        let k2 = i1 / DAYS_PHASE2 as f64;
        i1 - k2 * n_phase2 as f64
    };
    daily_issuance.max(0.0)
}

// ====================================================================================================
// 请求体结构体
// ====================================================================================================
#[derive(Deserialize)]
pub struct TriggerSettlementRequest {
    pub date: Option<String>,
}
#[derive(Deserialize)]
pub struct ForceNtxControlRequest {
    pub date: Option<String>,
}

// ====================================================================================================
// 业务逻辑函数（可被 HTTP 路由和定时任务共用）
// ====================================================================================================
pub async fn trigger_daily_settlement_logic(
    db: web::Data<Database>,
    date: Option<String>,
) -> Result<(), String> {
    let trade_date_str = date.unwrap_or_else(get_settlement_trade_date_string);
    println!("Logic Info: trigger_daily_settlement - Starting settlement for trade date: {}", trade_date_str);

    let (platform_data, trades_for_settlement, exchanges_info, referral_map) = match (
        db.get_platform_data(),
        db.get_trades_and_user_info_for_date(&trade_date_str),
        db.get_exchanges(),
        db.get_all_referral_relationships_as_map(),
    ) {
        (Ok(pd), Ok(tr), Ok(ex), Ok(re)) => (
            pd,
            tr,
            ex.into_iter().map(|e| (e.id, e.mining_efficiency)).collect::<HashMap<_, _>>(),
            re,
        ),
        (Err(e), _, _, _) => return Err(format!("Failed to fetch platform data: {:?}", e)),
        (_, Err(e), _, _) => return Err(format!("Failed to fetch trade data: {:?}", e)),
        (_, _, Err(e), _) => return Err(format!("Failed to fetch exchange data: {:?}", e)),
        (_, _, _, Err(e)) => return Err(format!("Failed to fetch referral data: {:?}", e)),
    };

    if trades_for_settlement.is_empty() {
        println!("Logic Info: trigger_daily_settlement - No trades found for {}, skipping.", trade_date_str);
        return Ok(());
    }

    let trader_ids: Vec<i64> = trades_for_settlement.iter().map(|t| t.user_id).collect::<HashSet<_>>().into_iter().collect();

    let mut users_with_trading_downlines: HashSet<i64> = HashSet::new();
    for trade in &trades_for_settlement {
        if let Some(&inviter_id) = referral_map.get(&trade.user_id) {
            users_with_trading_downlines.insert(inviter_id);
        }
    }

    let mut final_earnings: HashMap<i64, DailyUserRebate> = HashMap::new();
    let mut commission_records: Vec<(i64, i64, f64, String, String)> = Vec::new();
    let mut broker_status_cache: HashMap<i64, bool> = HashMap::new();

    let mut user_aggregated_data: HashMap<i64, (f64, f64)> = HashMap::new();
    for trade in &trades_for_settlement {
        let entry = user_aggregated_data.entry(trade.user_id).or_insert((0.0, 0.0));
        entry.0 += trade.fee_usdt;
        let exchange_efficiency = exchanges_info.get(&trade.exchange_id).cloned().unwrap_or(0.0) / 100.0;
        entry.1 += trade.fee_usdt * exchange_efficiency;
    }

    let platform_total_fees_for_day: f64 = user_aggregated_data.values().map(|(fee, _)| *fee).sum();
    let total_trading_volume_today: f64 = trades_for_settlement.iter().map(|t| t.trade_volume_usdt).sum();
    let daily_ntx_supply_for_today = get_daily_ntx_issuance(&trade_date_str, &platform_data.genesis_date);

    for (trader_id, (total_fee, raw_usdt_rebate_from_exchange)) in user_aggregated_data.iter() {
        let trader_id = *trader_id;
        let total_fee = *total_fee;
        let raw_usdt_rebate_from_exchange = *raw_usdt_rebate_from_exchange;

        let user_earning_entry = final_earnings.entry(trader_id).or_default();
        user_earning_entry.total_fees_incurred += total_fee;

        let has_trading_downline_today = users_with_trading_downlines.contains(&trader_id);
        let is_trader_broker = *broker_status_cache
            .entry(trader_id)
            .or_insert_with(|| db.is_broker(trader_id).unwrap_or(false));

        let user_actual_usdt_rebate = raw_usdt_rebate_from_exchange * 0.60;

        // 只有当是经纪商或者当天有交易的下属时，才获得 USDT 返佣
        if has_trading_downline_today || is_trader_broker {
            user_earning_entry.usdt_rebate += user_actual_usdt_rebate;
        }

        let ntx_rebate_total = if platform_total_fees_for_day > 0.0 {
            (total_fee / platform_total_fees_for_day) * daily_ntx_supply_for_today
        } else { 0.0 };

        let user_ntx_share = ntx_rebate_total * 0.90;
        let inviter_ntx_share = ntx_rebate_total * 0.10;

        user_earning_entry.ntx_rebate += user_ntx_share;

        if let Some(&inviter_id) = referral_map.get(&trader_id) {
            if inviter_ntx_share > 0.0 {
                let inviter_earning_entry = final_earnings.entry(inviter_id).or_default();
                inviter_earning_entry.ntx_bonus_earned += inviter_ntx_share;
                commission_records.push((inviter_id, trader_id, inviter_ntx_share, "NTX".to_string(), trade_date_str.clone()));
            }
        }

        let mut bonus_20_pct_claimed = false;
        let mut platform_bonus_10_pct_claimed = false;
        let mut current_user_id = trader_id;
        let mut is_first_level = true;

        while let Some(&inviter_id) = referral_map.get(&current_user_id) {
            let is_inviter_broker = *broker_status_cache
                .entry(inviter_id)
                .or_insert_with(|| db.is_broker(inviter_id).unwrap_or(false));

            if is_first_level {
                let usdt_bonus = raw_usdt_rebate_from_exchange * 0.30;
                if usdt_bonus > 0.0 {
                    let inviter_earning_entry = final_earnings.entry(inviter_id).or_default();
                    inviter_earning_entry.usdt_bonus_earned += usdt_bonus;
                    commission_records.push((inviter_id, trader_id, usdt_bonus, "USDT".to_string(), trade_date_str.clone()));
                }
            }

            if !bonus_20_pct_claimed && is_inviter_broker {
                let usdt_bonus = raw_usdt_rebate_from_exchange * 0.20;
                if usdt_bonus > 0.0 {
                    let inviter_earning_entry = final_earnings.entry(inviter_id).or_default();
                    inviter_earning_entry.usdt_bonus_earned += usdt_bonus;
                    commission_records.push((inviter_id, trader_id, usdt_bonus, "USDT".to_string(), trade_date_str.clone()));
                }
                bonus_20_pct_claimed = true;
            }

            let is_current_user_broker = *broker_status_cache
                .entry(current_user_id)
                .or_insert_with(|| db.is_broker(current_user_id).unwrap_or(false));

            if !platform_bonus_10_pct_claimed && is_current_user_broker {
                let usdt_bonus = raw_usdt_rebate_from_exchange * 0.10;
                if usdt_bonus > 0.0 {
                    let platform_bonus_recipient_entry = final_earnings.entry(inviter_id).or_default();
                    platform_bonus_recipient_entry.usdt_bonus_earned += usdt_bonus;
                    commission_records.push((inviter_id, trader_id, usdt_bonus, "USDT".to_string(), trade_date_str.clone()));
                }
                platform_bonus_10_pct_claimed = true;
            }

            current_user_id = inviter_id;
            is_first_level = false;

            if bonus_20_pct_claimed && platform_bonus_10_pct_claimed {
                break;
            }
        }
    }

    let total_ntx_distributed = final_earnings.values().map(|e| e.ntx_rebate + e.ntx_bonus_earned).sum();
    let total_usdt_commissions = final_earnings.values().map(|e| e.usdt_rebate + e.usdt_bonus_earned).sum();
    let all_involved_user_ids: HashSet<i64> = final_earnings.keys().cloned().collect();

    match db.perform_daily_settlement(
        &trade_date_str,
        &final_earnings,
        &commission_records,
        total_ntx_distributed,
        total_usdt_commissions,
        all_involved_user_ids.len() as i64,
        total_trading_volume_today,
    ) {
        Ok(_) => {
            println!("Logic Success: trigger_daily_settlement - Daily settlement for {} executed successfully.", trade_date_str);
            Ok(())
        }
        Err(e) => {
            eprintln!("Logic Error: trigger_daily_settlement - Database update failed during settlement for {}: {:?}", trade_date_str, e);
            Err("Database update failed during settlement.".to_string())
        }
    }
}

pub async fn force_ntx_control_logic(
    db: web::Data<Database>,
    date: Option<String>,
) -> Result<(), String> {
    let trade_date_str = date.unwrap_or_else(get_settlement_trade_date_string);
    println!("Logic Info: force_ntx_control - Starting NTX control for date: {}", trade_date_str);

    let target_percentage = match db.get_ntx_control_percentage() {
        Ok(p) => p,
        Err(e) => return Err(format!("Failed to get control percentage: {:?}", e)),
    };

    if !(0.0..100.0).contains(&target_percentage) {
        return Err(format!("Invalid target percentage configured in database: {}", target_percentage));
    }

    let (current_admin_fees, current_total_fees) = match (
        db.get_total_fees_for_date(&trade_date_str, true),
        db.get_total_fees_for_date(&trade_date_str, false)
    ) {
        (Ok(admin_fees), Ok(total_fees)) => (admin_fees, total_fees),
        _ => return Err("Failed to calculate current fees".to_string()),
    };

    let non_admin_fees = current_total_fees - current_admin_fees;
    let required_admin_fees = (target_percentage * non_admin_fees) / (100.0 - target_percentage);
    let additional_admin_fees = required_admin_fees - current_admin_fees;

    println!("Logic Info: force_ntx_control - Target: {}%, Current Admin Fees: {}, Non-Admin Fees: {}, Required Admin Fees: {}, Additional Fees Needed: {}",
        target_percentage, current_admin_fees, non_admin_fees, required_admin_fees, additional_admin_fees);

    if additional_admin_fees <= 0.0 {
        let current_percentage = if current_total_fees > 0.0 { (current_admin_fees / current_total_fees) * 100.0 } else { 100.0 };
        println!("Logic Info: force_ntx_control - Admin fee percentage ({:.2}%) already meets or exceeds target ({}%). No action taken.", current_percentage, target_percentage);
        return Ok(());
    }

    let admin_ids = match db.get_all_admin_user_ids() {
        Ok(ids) if !ids.is_empty() => ids,
        Ok(_) => return Err("No admin users found.".to_string()),
        Err(e) => return Err(format!("Failed to get admin users: {:?}", e)),
    };

    let fee_per_admin = additional_admin_fees / admin_ids.len() as f64;
    let volume_per_admin = fee_per_admin * 2000.0;
    let default_exchange_id = 1;
    let default_exchange_name = db.get_exchange_name_by_id(default_exchange_id).unwrap_or(Some("Bitget".to_string())).unwrap();

    let mut fake_trades: Vec<FakeTradeData> = Vec::new();

    for admin_id in admin_ids {
        let admin_email = match db.get_user_email_by_id(admin_id) {
            Ok(Some(email)) => email,
            _ => {
                eprintln!("Warning: force_ntx_control - Could not find email for admin ID {}, skipping.", admin_id);
                continue;
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

    if fake_trades.is_empty() {
        println!("Logic Info: force_ntx_control - No valid admins to process. No trades were added.");
        return Ok(());
    }

    match db.add_fake_admin_trades_in_transaction(&fake_trades) {
        Ok(_) => {
            println!("Logic Success: force_ntx_control - Successfully added {:.4} USDT in fees across {} admin(s) for date {}.",
                additional_admin_fees, fake_trades.len(), trade_date_str);
            Ok(())
        }
        Err(e) => {
            eprintln!("Logic Error: force_ntx_control - Database update failed during NTX control: {:?}", e);
            Err("Database update failed during NTX control.".to_string())
        }
    }
}

// ====================================================================================================
// Actix 路由处理函数（仅做参数解析和响应，调用上面逻辑函数）
// ====================================================================================================
#[post("/trigger_daily_settlement")]
pub async fn trigger_daily_settlement(
    db: web::Data<Database>,
    payload: web::Json<TriggerSettlementRequest>,
) -> impl Responder {
    match trigger_daily_settlement_logic(db, payload.date.clone()).await {
        Ok(_) => HttpResponse::Ok().json(serde_json::json!({"message": "Daily settlement successful."})),
        Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({"error": e})),
    }
}

#[post("/force_ntx_control")]
pub async fn force_ntx_control(
    db: web::Data<Database>,
    payload: web::Json<ForceNtxControlRequest>,
) -> impl Responder {
    match force_ntx_control_logic(db, payload.date.clone()).await {
        Ok(_) => HttpResponse::Ok().json(serde_json::json!({"message": "NTX control operation successful."})),
        Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({"error": e})),
    }
}