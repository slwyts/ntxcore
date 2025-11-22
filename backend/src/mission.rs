use actix_web::{get, post, web, HttpResponse, Responder, HttpRequest};
use serde::{Deserialize, Serialize};
use crate::db::Database;
use crate::JwtConfig;
use crate::user::get_user_id_from_token;
use rusqlite::{params, OptionalExtension};
use crate::utils::get_current_utc_time_string;
use chrono::{Utc, Duration};

#[derive(Serialize, Deserialize, Debug)]
pub struct Task {
    pub id: i64,
    pub name: String,
    pub description: String,
    pub reward_amount: f64,
    pub task_type: String,
    pub condition_value: i64,
    pub is_daily: bool,
    pub is_active: bool,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct UserTaskStatus {
    pub task_id: i64,
    pub status: String, // 'NOT_STARTED', 'COMPLETED', 'CLAIMED'
    pub progress: i64,
}

#[derive(Serialize, Debug)]
pub struct TaskWithStatus {
    #[serde(flatten)]
    pub task: Task,
    pub status: String,
    pub progress: i64,
}

#[derive(Deserialize)]
pub struct ClaimRewardRequest {
    pub task_id: i64,
}

#[derive(Deserialize)]
pub struct ReportActionRequest {
    pub action: String, // 'daily_live', 'daily_share'
}

// 获取任务列表
#[get("/list")]
pub async fn get_tasks(
    db: web::Data<Database>,
    jwt_config: web::Data<JwtConfig>,
    req: HttpRequest,
) -> impl Responder {
    let user_id = match get_user_id_from_token(&req, &jwt_config) {
        Ok(id) => id,
        Err(_) => return HttpResponse::Unauthorized().finish(),
    };

    let conn = db.conn.lock().unwrap();
    
    // 1. 获取所有激活的任务
    let mut stmt = conn.prepare("SELECT id, name, description, reward_amount, task_type, condition_value, is_daily, is_active FROM tasks WHERE is_active = TRUE").unwrap();
    let tasks_iter = stmt.query_map([], |row| {
        Ok(Task {
            id: row.get(0)?,
            name: row.get(1)?,
            description: row.get(2)?,
            reward_amount: row.get(3)?,
            task_type: row.get(4)?,
            condition_value: row.get(5)?,
            is_daily: row.get(6)?,
            is_active: row.get(7)?,
        })
    }).unwrap();

    let mut tasks_with_status = Vec::new();

    for task in tasks_iter {
        let task = task.unwrap();
        
        // 2. 获取用户任务状态
        // 对于每日任务，需要检查 updated_at 是否是今天
        let mut status = "NOT_STARTED".to_string();
        let mut progress = 0;

        let user_task_row: Option<(String, i64, String)> = conn.query_row(
            "SELECT status, progress, updated_at FROM user_task_progress WHERE user_id = ? AND task_id = ?",
            params![user_id, task.id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        ).optional().unwrap();

        if let Some((db_status, db_progress, updated_at)) = user_task_row {
            if task.is_daily {
                // 检查日期
                let today = get_current_utc_time_string()[0..10].to_string();
                let updated_date = updated_at[0..10].to_string();
                if today == updated_date {
                    status = db_status;
                    progress = db_progress;
                } else {
                    // 过期了，重置
                    status = "NOT_STARTED".to_string();
                    progress = 0;
                }
            } else {
                status = db_status;
                progress = db_progress;
            }
        }

        // 3. 实时检查任务完成情况 (如果状态不是 CLAIMED)
        if status != "CLAIMED" {
            let (new_status, new_progress) = check_task_completion(&conn, user_id, &task, &status, progress);
            if new_status != status || new_progress != progress {
                status = new_status.clone();
                progress = new_progress;
                // 更新数据库 (如果是 COMPLETED)
                if status == "COMPLETED" {
                     let _ = conn.execute(
                        "INSERT INTO user_task_progress (user_id, task_id, status, progress, updated_at) VALUES (?, ?, ?, ?, ?)
                         ON CONFLICT(user_id, task_id) DO UPDATE SET status = ?, progress = ?, updated_at = ?",
                        params![user_id, task.id, status, progress, get_current_utc_time_string(), status, progress, get_current_utc_time_string()]
                    );
                }
            }
        }

        tasks_with_status.push(TaskWithStatus {
            task,
            status,
            progress,
        });
    }

    HttpResponse::Ok().json(tasks_with_status)
}

// 领取奖励
#[post("/claim")]
pub async fn claim_reward(
    db: web::Data<Database>,
    jwt_config: web::Data<JwtConfig>,
    req: HttpRequest,
    body: web::Json<ClaimRewardRequest>,
) -> impl Responder {
    let user_id = match get_user_id_from_token(&req, &jwt_config) {
        Ok(id) => id,
        Err(_) => return HttpResponse::Unauthorized().finish(),
    };

    let mut conn = db.conn.lock().unwrap();
    let tx = conn.transaction().unwrap();

    // 1. 检查任务是否存在且已完成但未领取
    let task: Option<Task> = tx.query_row(
        "SELECT id, name, description, reward_amount, task_type, condition_value, is_daily, is_active FROM tasks WHERE id = ?",
        params![body.task_id],
        |row| {
            Ok(Task {
                id: row.get(0)?,
                name: row.get(1)?,
                description: row.get(2)?,
                reward_amount: row.get(3)?,
                task_type: row.get(4)?,
                condition_value: row.get(5)?,
                is_daily: row.get(6)?,
                is_active: row.get(7)?,
            })
        }
    ).optional().unwrap();

    if task.is_none() {
        return HttpResponse::BadRequest().json(serde_json::json!({"error": "任务不存在"}));
    }
    let task = task.unwrap();

    let user_task_row: Option<(String, String)> = tx.query_row(
        "SELECT status, updated_at FROM user_task_progress WHERE user_id = ? AND task_id = ?",
        params![user_id, task.id],
        |row| Ok((row.get(0)?, row.get(1)?))
    ).optional().unwrap();

    let mut can_claim = false;
    if let Some((status, updated_at)) = user_task_row {
        if status == "COMPLETED" {
            if task.is_daily {
                let today = get_current_utc_time_string()[0..10].to_string();
                let updated_date = updated_at[0..10].to_string();
                if today == updated_date {
                    can_claim = true;
                }
            } else {
                can_claim = true;
            }
        }
    }

    if !can_claim {
        return HttpResponse::BadRequest().json(serde_json::json!({"error": "任务未完成或已领取"}));
    }

    // 2. 发放奖励
    // 更新用户 NTX 余额
    let _ = tx.execute(
        "UPDATE users SET ntx_balance = ntx_balance + ? WHERE id = ?",
        params![task.reward_amount, user_id]
    );

    // 3. 更新任务状态为 CLAIMED
    let _ = tx.execute(
        "UPDATE user_task_progress SET status = 'CLAIMED', updated_at = ? WHERE user_id = ? AND task_id = ?",
        params![get_current_utc_time_string(), user_id, task.id]
    );

    tx.commit().unwrap();

    HttpResponse::Ok().json(serde_json::json!({"message": "领取成功", "reward": task.reward_amount}))
}

// 上报行为 (分享、观看直播)
#[post("/action")]
pub async fn report_action(
    db: web::Data<Database>,
    jwt_config: web::Data<JwtConfig>,
    req: HttpRequest,
    body: web::Json<ReportActionRequest>,
) -> impl Responder {
    let user_id = match get_user_id_from_token(&req, &jwt_config) {
        Ok(id) => id,
        Err(_) => return HttpResponse::Unauthorized().finish(),
    };

    let conn = db.conn.lock().unwrap();
    
    // 找到对应的任务类型
    let task_type = match body.action.as_str() {
        "daily_live" => "DAILY_LIVE",
        "daily_share" => "DAILY_SHARE",
        _ => return HttpResponse::BadRequest().json(serde_json::json!({"error": "无效的行为"})),
    };

    // 查找该类型的任务
    let task_id: Option<i64> = conn.query_row(
        "SELECT id FROM tasks WHERE task_type = ? AND is_active = TRUE",
        params![task_type],
        |row| row.get(0)
    ).optional().unwrap();

    if let Some(tid) = task_id {
        // 标记为完成
        // 对于每日任务，直接更新为 COMPLETED (如果还没 CLAIMED)
        // 检查是否已经 CLAIMED 今天
        let status: Option<(String, String)> = conn.query_row(
            "SELECT status, updated_at FROM user_task_progress WHERE user_id = ? AND task_id = ?",
            params![user_id, tid],
            |row| Ok((row.get(0)?, row.get(1)?))
        ).optional().unwrap();

        let mut already_done_today = false;
        if let Some((s, updated_at)) = status {
             let today = get_current_utc_time_string()[0..10].to_string();
             let updated_date = updated_at[0..10].to_string();
             if today == updated_date && (s == "COMPLETED" || s == "CLAIMED") {
                 already_done_today = true;
             }
        }

        if !already_done_today {
             let _ = conn.execute(
                "INSERT INTO user_task_progress (user_id, task_id, status, progress, updated_at) VALUES (?, ?, 'COMPLETED', 1, ?)
                 ON CONFLICT(user_id, task_id) DO UPDATE SET status = 'COMPLETED', progress = 1, updated_at = ?",
                params![user_id, tid, get_current_utc_time_string(), get_current_utc_time_string()]
            );
        }
    }

    HttpResponse::Ok().json(serde_json::json!({"message": "行为已记录"}))
}


// 辅助函数：检查任务完成情况
fn check_task_completion(conn: &rusqlite::Connection, user_id: i64, task: &Task, current_status: &str, current_progress: i64) -> (String, i64) {
    if current_status == "COMPLETED" || current_status == "CLAIMED" {
        return (current_status.to_string(), current_progress);
    }

    match task.task_type.as_str() {
        "REGISTER" => {
            // 只要用户存在就是完成了
            ("COMPLETED".to_string(), 1)
        },
        "BIND_EXCHANGE" => {
            // 检查 user_exchanges 表
            let count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM user_exchanges WHERE userId = ? AND isBound = 1",
                params![user_id],
                |row| row.get(0)
            ).unwrap_or(0);
            if count > 0 {
                ("COMPLETED".to_string(), 1)
            } else {
                ("NOT_STARTED".to_string(), 0)
            }
        },
        "REFERRAL_COUNT" => {
            // 获取用户邮箱
            let user_email: String = conn.query_row("SELECT email FROM users WHERE id = ?", params![user_id], |row| row.get(0)).unwrap_or_default();
            
            let count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM users WHERE inviteBy = ?",
                params![user_email],
                |row| row.get(0)
            ).unwrap_or(0);

            if count >= task.condition_value {
                ("COMPLETED".to_string(), count)
            } else {
                ("IN_PROGRESS".to_string(), count)
            }
        },
        "TEAM_SIZE" => {
            // 计算团队总人数（包含所有裂变层级）
            let team_size = calculate_team_size(conn, user_id);
            if team_size >= task.condition_value {
                ("COMPLETED".to_string(), team_size)
            } else {
                ("IN_PROGRESS".to_string(), team_size)
            }
        },
        "TRADE_ACTIVITY" => {
            // 检查昨天是否有交易 (用户或下级)
            // 获取昨天的日期 YYYY-MM-DD
            let yesterday = (Utc::now() - Duration::days(1)).format("%Y-%m-%d").to_string();
            
            // 1. 检查用户自己
            let self_trade_count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM daily_user_trades WHERE user_id = ? AND trade_date = ?",
                params![user_id, yesterday],
                |row| row.get(0)
            ).unwrap_or(0);

            if self_trade_count > 0 {
                return ("COMPLETED".to_string(), 1);
            }

            // 2. 检查直推下级
            // 获取用户邮箱
            let user_email: Option<String> = conn.query_row(
                "SELECT email FROM users WHERE id = ?",
                params![user_id],
                |row| row.get(0)
            ).optional().unwrap_or(None);

            if let Some(email) = user_email {
                let sub_trade_count: i64 = conn.query_row(
                    "SELECT COUNT(*) FROM daily_user_trades dt
                     JOIN users u ON dt.user_id = u.id
                     WHERE u.inviteBy = ? AND dt.trade_date = ?",
                    params![email, yesterday],
                    |row| row.get(0)
                ).unwrap_or(0);

                if sub_trade_count > 0 {
                    ("COMPLETED".to_string(), 1)
                } else {
                    ("NOT_STARTED".to_string(), 0)
                }
            } else {
                ("NOT_STARTED".to_string(), 0)
            }
        },
        _ => (current_status.to_string(), current_progress),
    }
}

fn calculate_team_size(conn: &rusqlite::Connection, user_id: i64) -> i64 {
    // 获取用户邮箱
    let user_email: Option<String> = conn.query_row(
        "SELECT email FROM users WHERE id = ?",
        params![user_id],
        |row| row.get(0)
    ).optional().unwrap_or(None);

    if let Some(email) = user_email {
        // 使用递归查询 (CTE) 计算所有下级（无限层级裂变）
        // 注意：inviteBy 存储的是邀请人的邮箱
        let query = "
            WITH RECURSIVE subordinates AS (
                SELECT id, email FROM users WHERE inviteBy = ?
                UNION ALL
                SELECT u.id, u.email FROM users u
                INNER JOIN subordinates s ON u.inviteBy = s.email
            )
            SELECT COUNT(*) FROM subordinates;
        ";
        conn.query_row(query, params![email], |row| row.get(0)).unwrap_or(0)
    } else {
        0
    }
}
