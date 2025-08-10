use tokio::time::{sleep_until, Duration, Instant};
use chrono::{Local, NaiveTime};
use chrono::Timelike;
use actix_web::web::Data;
use crate::db::Database;
// use crate::settlement::{trigger_daily_settlement, force_ntx_control};

pub async fn start_scheduled_tasks(db: Data<Database>) {
    let db_clone = db.clone();
    tokio::spawn(async move {
        if let Err(e) = schedule_task("DAILY_SETTLEMENT_TIME", db_clone.clone(), trigger_daily_settlement_task).await {
            eprintln!("每日结算任务失败: {}", e);
        }
    });

    let db_clone = db.clone();
    tokio::spawn(async move {
        if let Err(e) = schedule_task("NTX_CONTROL_TIME", db_clone.clone(), force_ntx_control_task).await {
            eprintln!("NTX分配控制任务失败: {}", e);
        }
    });
}

async fn schedule_task<F>(
    env_key: &str,
    db: Data<Database>,
    task_fn: F,
) -> Result<(), Box<dyn std::error::Error>>
where
    F: Fn(Data<Database>) -> Result<(), Box<dyn std::error::Error>> + Send + 'static,
{
    let time_str = std::env::var(env_key).unwrap_or_else(|_| "00:00".to_string());
    let task_time = NaiveTime::parse_from_str(&time_str, "%H:%M")?;
    let now = Local::now().naive_local();
    let today = now.date();
    let today_task_time = today.and_hms_opt(
        task_time.hour(),
        task_time.minute(),
        0
    ).ok_or_else(|| Box::<dyn std::error::Error>::from("Invalid task time"))?;

    let next_run = if now > today_task_time {
        today_task_time + chrono::Duration::days(1)
    } else {
        today_task_time
    };

    let duration_until_next_run = (next_run - now).to_std()?;
    sleep_until(Instant::now() + duration_until_next_run).await;

    loop {
        task_fn(db.clone())?;
        sleep_until(Instant::now() + Duration::from_secs(24 * 60 * 60)).await;
    }
}

fn trigger_daily_settlement_task(db: Data<Database>) -> Result<(), Box<dyn std::error::Error>> {
    // 调用每日结算逻辑，使用默认时间（昨天）
    tokio::spawn(async move {
        // 直接调用业务逻辑函数而不是 actix handler
        if let Err(e) = crate::settlement::trigger_daily_settlement_logic(db, None).await {
            eprintln!("每日结算逻辑失败: {}", e);
        }
    });
    Ok(())
}

fn force_ntx_control_task(db: Data<Database>) -> Result<(), Box<dyn std::error::Error>> {
    // 调用NTX分配控制逻辑，使用默认时间（昨天）
    tokio::spawn(async move {
        // 直接调用业务逻辑函数而不是 actix handler
        if let Err(e) = crate::settlement::force_ntx_control_logic(db, None).await {
            eprintln!("NTX分配控制逻辑失败: {}", e);
        }
    });
    Ok(())
}