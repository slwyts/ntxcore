// src/user.rs
use actix_web::{get, post, web, HttpResponse, Responder, HttpRequest, put};
use serde::{Deserialize, Serialize};
use crate::db::{Database, UserInfo}; 
use crate::utils::{get_current_utc_time_string, is_valid_evm_address};
use jsonwebtoken::{decode, DecodingKey, Validation, Algorithm};
use rusqlite::{params, Error as RusqliteError}; 
use crate::JwtConfig;
use crate::auth::Claims;
 // 导入新增的结构体 WithdrawalOrder

//edit user nickname 
#[derive(Deserialize)]
pub struct UpdateNicknameRequest {
    pub nickname: String,
}

// 用户信息响应结构体 (MODIFIED)
#[derive(Serialize)]
pub struct UserInfoResponse {
    #[serde(rename = "id")]
    pub id: i64,
    #[serde(rename = "nickname")]
    pub nickname: String,
    #[serde(rename = "email")]
    pub email: String,
    #[serde(rename = "myInviteCode")]
    pub my_invite_code: String,
    #[serde(rename = "invitedBy")]
    pub invited_by: Option<String>,
    pub exp: i64,
    #[serde(rename = "role")] // Broker 或 Normal User
    pub role: String,
    #[serde(rename = "usdtBalance")]
    pub usdt_balance: f64,
    #[serde(rename = "ntxBalance")]
    pub ntx_balance: f64,
    #[serde(rename = "bscAddress")]
    pub bsc_address: Option<String>,
    #[serde(rename = "gntxBalance")]
    pub gntx_balance: f64,
    #[serde(rename = "invitedUserCount")]
    pub invited_user_count: i64,
}


// 提现请求体
#[derive(Deserialize)]
pub struct WithdrawRequest {
    pub amount: i64,
    #[serde(rename = "toAddress")]
    pub to_address: String,
}

// 绑定 BSC 地址请求体 (新增)
#[derive(Deserialize)]
pub struct BindBscAddressRequest {
    #[serde(rename = "bscAddress")]
    pub bsc_address: String,
}

// 获取当前 DAO 拍卖状态的响应结构体 (新增)
#[derive(Serialize)]
pub struct CurrentDaoAuctionResponse {
    #[serde(rename = "isAuctionInProgress")]
    pub is_auction_in_progress: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "startTime")]
    pub start_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "endTime")]
    pub end_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "adminBscAddress")]
    pub admin_bsc_address: Option<String>,
}


// 辅助函数：从请求头中获取用户ID
pub fn get_user_id_from_token(req: &HttpRequest, jwt_config: &JwtConfig) -> Result<i64, HttpResponse> {
    let auth_header = req.headers().get("Authorization");

    let token_str = match auth_header {
        Some(header_value) => {
            let header_str = match header_value.to_str() {
                Ok(s) => s,
                Err(_) => {
                    eprintln!("API Error: get_user_id_from_token - Authorization header is not valid UTF-8.");
                    return Err(HttpResponse::BadRequest().json(
                        serde_json::json!({"error": "无效的Authorization头部"})
                    ));
                },
            };
            if header_str.starts_with("Bearer ") {
                header_str.trim_start_matches("Bearer ").to_string()
            } else {
                eprintln!("API Error: get_user_id_from_token - Authorization header does not start with 'Bearer '.");
                return Err(HttpResponse::Unauthorized().json(
                    serde_json::json!({"error": "未授权：无效的token格式"})
                ));
            }
        },
        None => {
            eprintln!("API Error: get_user_id_from_token - Authorization header is missing.");
            return Err(HttpResponse::Unauthorized().json(
                serde_json::json!({"error": "未授权：缺少token"})
            ));
        },
    };

    let decoding_key = DecodingKey::from_secret(jwt_config.secret.as_bytes());
    let validation = Validation::new(Algorithm::HS256);

    let token_data = match decode::<Claims>(&token_str, &decoding_key, &validation) {
        Ok(data) => data,
        Err(e) => {
            eprintln!("API Error: get_user_id_from_token - Token decoding failed: {:?}", e);
            return Err(HttpResponse::Unauthorized().json(
                serde_json::json!({"error": "未授权：无效的token"})
            ));
        },
    };

    Ok(token_data.claims.sub)
}


// 获取用户信息接口 (MODIFIED)
#[get("/get_user_info")]
pub async fn get_user_info(
    db: web::Data<Database>,
    jwt_config: web::Data<JwtConfig>,
    req: HttpRequest,
) -> impl Responder {
    println!("API Call: /api/user/get_user_info received.");

    let user_id = match get_user_id_from_token(&req, &jwt_config) {
        Ok(id) => id,
        Err(resp) => {
            eprintln!("API Error: /api/user/get_user_info - 未授权访问。");
            return resp;
        },
    };
    println!("API Info: /api/user/get_user_info - 用户ID {} 请求信息。", user_id);

    match db.get_user_info(user_id) {
        Ok(Some(user_db_info)) => {
            println!("API Success: /api/user/get_user_info - 已获取用户 {} 的信息。", user_id);

            // 获取用户绑定的 BSC 地址
            let bsc_address = match db.get_user_bsc_address(user_id) {
                Ok(addr) => addr,
                Err(e) => {
                    eprintln!("API Error: /api/user/get_user_info - 获取用户 {} 的 BSC 地址失败: {:?}", user_id, e);
                    None
                }
            };
            
            // --- 新增逻辑：获取已邀请的用户数量 ---
            let invited_user_count = match db.get_invited_user_count_by_email(&user_db_info.email) {
                Ok(count) => count,
                Err(e) => {
                    eprintln!("API Error: /api/user/get_user_info - 获取用户 {} 的邀请数量失败: {:?}", user_id, e);
                    0 // 如果查询失败，默认为0
                }
            };
            // --- 逻辑结束 ---

            // 确定用户角色
            let is_broker = db.is_broker(user_id).unwrap_or(false);
            let role = if is_broker { "Broker".to_string() } else { "Normal User".to_string() };

            HttpResponse::Ok().json(UserInfoResponse {
                id: user_db_info.id,
                nickname: user_db_info.nickname,
                email: user_db_info.email,
                my_invite_code: user_db_info.my_invite_code,
                invited_by: user_db_info.invited_by,
                exp: user_db_info.exp,
                role,
                usdt_balance: user_db_info.usdt_balance,
                ntx_balance: user_db_info.ntx_balance,
                bsc_address,
                gntx_balance: user_db_info.gntx_balance,
                invited_user_count,
            })
        },
        Ok(None) => {
            eprintln!("API Error: /api/user/get_user_info - 未找到用户ID {}。", user_id);
            HttpResponse::NotFound().json(
                serde_json::json!({"error": "未找到该用户"})
            )
        },
        Err(e) => {
            eprintln!("API Error: /api/user/get_user_info - 获取用户 {} 信息失败: {:?}", user_id, e);
            HttpResponse::InternalServerError().finish()
        },
    }
}


// 用户提现 USDT 接口
#[post("/want_withdraw_usdt")]
pub async fn want_withdraw_usdt(
    db: web::Data<Database>,
    jwt_config: web::Data<JwtConfig>,
    req: HttpRequest,
    withdraw_req: web::Json<WithdrawRequest>,
) -> impl Responder {
    println!("API Call: /api/user/want_withdraw_usdt received for amount: {}", withdraw_req.amount);

    let user_id = match get_user_id_from_token(&req, &jwt_config) {
        Ok(id) => id,
        Err(resp) => {
            eprintln!("API Error: /api/user/want_withdraw_usdt - 未授权访问。");
            return resp;
        },
    };
    println!("API Info: /api/user/want_withdraw_usdt - 用户ID {} 请求提现USDT。", user_id);

    if withdraw_req.amount <= 0 {
        eprintln!("API Error: /api/user/want_withdraw_usdt - 用户 {} 提现金额无效: {}", user_id, withdraw_req.amount);
        return HttpResponse::BadRequest().json(
            serde_json::json!({"error": "提现金额必须大于0"})
        );
    }

    if !is_valid_evm_address(&withdraw_req.to_address) {
        eprintln!("API Error: /api/user/want_withdraw_usdt - 用户 {} 提现地址无效: {}", user_id, withdraw_req.to_address);
        return HttpResponse::BadRequest().json(
            serde_json::json!({"error": "提现地址无效，请提供有效的EVM地址"})
        );
    }

    let conn_mutex = db.conn.clone();
    let mut conn = conn_mutex.lock().unwrap(); // 获取数据库连接锁
    let tx = match conn.transaction() { // 启动事务
        Ok(t) => t,
        Err(e) => {
            eprintln!("API Error: /api/user/want_withdraw_usdt - 用户 {} 开启事务失败: {:?}", user_id, e);
            return HttpResponse::InternalServerError().finish();
        },
    };

    // <<<<<< MODIFIED SECTION START >>>>>>
    // 直接使用事务 tx 查询用户信息，避免重入锁
    let user_info = match tx.query_row(
        "SELECT id, nickname, email, inviteCode, inviteBy, exp, usdt_balance, ntx_balance, is_active, gntx_balance FROM users WHERE id = ?",
        params![user_id],
        |row| {
            Ok(UserInfo { // crate::db::UserInfo
                id: row.get(0)?,
                nickname: row.get(1)?,
                email: row.get(2)?,
                my_invite_code: row.get(3)?,
                invited_by: row.get(4)?,
                exp: row.get(5)?,
                usdt_balance: row.get(6)?,
                ntx_balance: row.get(7)?,
                is_active: row.get(8)?,
                gntx_balance: row.get(9)?,
            })
        },
    ) {
        Ok(info) => info,
        Err(RusqliteError::QueryReturnedNoRows) => {
            eprintln!("API Error: /api/user/want_withdraw_usdt - 未找到用户ID {}。", user_id);
            // 事务会自动回滚如果 tx 被丢弃且未提交
            return HttpResponse::NotFound().json(
                serde_json::json!({"error": "未找到该用户"})
            );
        },
        Err(e) => {
            eprintln!("API Error: /api/user/want_withdraw_usdt - 获取用户 {} 信息失败: {:?}", user_id, e);
            return HttpResponse::InternalServerError().finish();
        },
    };
    // <<<<<< MODIFIED SECTION END >>>>>>

    if user_info.usdt_balance < withdraw_req.amount as f64 {
        eprintln!("API Error: /api/user/want_withdraw_usdt - 用户 {} USDT余额不足。余额: {}, 提现: {}", user_id, user_info.usdt_balance, withdraw_req.amount);
        return HttpResponse::BadRequest().json(
            serde_json::json!({"error": "USDT余额不足"})
        );
    }

    let new_usdt_balance = user_info.usdt_balance - withdraw_req.amount as f64;
    if let Err(e) = tx.execute(
        "UPDATE users SET usdt_balance = ? WHERE id = ?",
        params![new_usdt_balance, user_id],
    ) {
        eprintln!("API Error: /api/user/want_withdraw_usdt - 扣除用户 {} USDT余额失败: {:?}", user_id, e);
        return HttpResponse::InternalServerError().finish();
    }

    let current_time = get_current_utc_time_string();
    if let Err(e) = tx.execute(
        "INSERT INTO withdrawal_orders (user_id, user_email, amount, currency, to_address, is_confirmed, created_at, status) VALUES (?, ?, ?, ?, ?, ?, ?, 'pending')",
        params![user_id, user_info.email, withdraw_req.amount, "USDT", withdraw_req.to_address, false, current_time],
    ) {
        eprintln!("API Error: /api/user/want_withdraw_usdt - 创建USDT提现订单失败 {}: {:?}", user_id, e);
        return HttpResponse::InternalServerError().finish();
    }

    if let Err(e) = tx.commit() {
        eprintln!("API Error: /api/user/want_withdraw_usdt - 提交事务失败 {}: {:?}", user_id, e);
        return HttpResponse::InternalServerError().finish();
    }

    println!("API Success: /api/user/want_withdraw_usdt - 用户 {} 成功申请提现 {} USDT 到 {}", user_id, withdraw_req.amount, withdraw_req.to_address);
    HttpResponse::Ok().json(
        serde_json::json!({"message": "USDT提现申请成功，等待管理员确认"})
    )
}

// 用户提现 NTX 接口
#[post("/want_withdraw_ntx")]
pub async fn want_withdraw_ntx(
    db: web::Data<Database>,
    jwt_config: web::Data<JwtConfig>,
    req: HttpRequest,
    withdraw_req: web::Json<WithdrawRequest>,
) -> impl Responder {
    println!("API Call: /api/user/want_withdraw_ntx received for amount: {}", withdraw_req.amount);

    let user_id = match get_user_id_from_token(&req, &jwt_config) {
        Ok(id) => id,
        Err(resp) => {
            eprintln!("API Error: /api/user/want_withdraw_ntx - 未授权访问。");
            return resp;
        },
    };
    println!("API Info: /api/user/want_withdraw_ntx - 用户ID {} 请求提现NTX。", user_id);

    if withdraw_req.amount <= 0 {
        eprintln!("API Error: /api/user/want_withdraw_ntx - 用户 {} 提现金额无效: {}", user_id, withdraw_req.amount);
        return HttpResponse::BadRequest().json(
            serde_json::json!({"error": "提现金额必须大于0"})
        );
    }

    if !is_valid_evm_address(&withdraw_req.to_address) {
        eprintln!("API Error: /api/user/want_withdraw_ntx - 用户 {} 提现地址无效: {}", user_id, withdraw_req.to_address);
        return HttpResponse::BadRequest().json(
            serde_json::json!({"error": "提现地址无效，请提供有效的EVM地址"})
        );
    }

    let conn_mutex = db.conn.clone();
    let mut conn = conn_mutex.lock().unwrap();
    let tx = match conn.transaction() {
        Ok(t) => t,
        Err(e) => {
            eprintln!("API Error: /api/user/want_withdraw_ntx - 用户 {} 开启事务失败: {:?}", user_id, e);
            return HttpResponse::InternalServerError().finish();
        },
    };
// <<<<<< MODIFIED SECTION START >>>>>>
    let user_info = match tx.query_row(
        // 在 SELECT 查询中增加 gntx_balance 字段
        "SELECT id, nickname, email, inviteCode, inviteBy, exp, usdt_balance, ntx_balance, is_active, gntx_balance FROM users WHERE id = ?",
        params![user_id],
        |row| {
            Ok(UserInfo { // crate::db::UserInfo
                id: row.get(0)?,
                nickname: row.get(1)?,
                email: row.get(2)?,
                my_invite_code: row.get(3)?,
                invited_by: row.get(4)?,
                exp: row.get(5)?,
                usdt_balance: row.get(6)?,
                ntx_balance: row.get(7)?,
                is_active: row.get(8)?,
                gntx_balance: row.get(9)?, // 这行已经正确
            })
        },
    ){
        Ok(info) => info,
        Err(RusqliteError::QueryReturnedNoRows) => {
            eprintln!("API Error: /api/user/want_withdraw_ntx - 未找到用户ID {}。", user_id);
            return HttpResponse::NotFound().json(
                serde_json::json!({"error": "未找到该用户"})
            );
        },
        Err(e) => {
            eprintln!("API Error: /api/user/want_withdraw_ntx - 获取用户 {} 信息失败: {:?}", user_id, e);
            return HttpResponse::InternalServerError().finish();
        },
    };
    // <<<<<< MODIFIED SECTION END >>>>>>

    if user_info.ntx_balance < withdraw_req.amount as f64 {
        eprintln!("API Error: /api/user/want_withdraw_ntx - 用户 {} NTX余额不足。余额: {}, 提现: {}", user_id, user_info.ntx_balance, withdraw_req.amount);
        return HttpResponse::BadRequest().json(
            serde_json::json!({"error": "NTX余额不足"})
        );
    }

    let new_ntx_balance = user_info.ntx_balance - withdraw_req.amount as f64;
    if let Err(e) = tx.execute(
        "UPDATE users SET ntx_balance = ? WHERE id = ?",
        params![new_ntx_balance, user_id],
    ) {
        eprintln!("API Error: /api/user/want_withdraw_ntx - 扣除用户 {} NTX余额失败: {:?}", user_id, e);
        return HttpResponse::InternalServerError().finish();
    }

    let current_time = get_current_utc_time_string();
    if let Err(e) = tx.execute(
        "INSERT INTO withdrawal_orders (user_id, user_email, amount, currency, to_address, is_confirmed, created_at, status) VALUES (?, ?, ?, ?, ?, ?, ?, 'pending')",
        params![user_id, user_info.email, withdraw_req.amount, "NTX", withdraw_req.to_address, false, current_time],
    ) {
        eprintln!("API Error: /api/user/want_withdraw_ntx - 创建NTX提现订单失败 {}: {:?}", user_id, e);
        return HttpResponse::InternalServerError().finish();
    }

    if let Err(e) = tx.commit() {
        eprintln!("API Error: /api/user/want_withdraw_ntx - 提交事务失败 {}: {:?}", user_id, e);
        return HttpResponse::InternalServerError().finish();
    }

    println!("API Success: /api/user/want_withdraw_ntx - 用户 {} 成功申请提现 {} NTX 到 {}", user_id, withdraw_req.amount, withdraw_req.to_address);
    HttpResponse::Ok().json(
        serde_json::json!({"message": "NTX提现申请成功，等待管理员确认"})
    )
}

// 获取我的团队信息
#[get("/my_teams")]
pub async fn get_my_teams(
    db: web::Data<Database>,
    jwt_config: web::Data<JwtConfig>,
    req: HttpRequest,
) -> impl Responder {
    println!("API Call: /api/user/my_teams received.");

    let user_id = match get_user_id_from_token(&req, &jwt_config) {
        Ok(id) => id,
        Err(resp) => {
            eprintln!("API Error: /api/user/my_teams - 未授权访问。");
            return resp;
        },
    };
    println!("API Info: /api/user/my_teams - 用户ID {} 请求我的团队信息。", user_id);

    // 获取用户自己的邀请码
    let my_invite_code = match db.get_user_info(user_id) {
        Ok(Some(user_info)) => user_info.my_invite_code,
        Ok(None) => {
            eprintln!("API Error: /api/user/my_teams - 未找到用户ID {}。", user_id);
            return HttpResponse::NotFound().json(
                serde_json::json!({"error": "未找到该用户"})
            );
        },
        Err(e) => {
            eprintln!("API Error: /api/user/my_teams - 获取用户 {} 信息失败: {:?}", user_id, e);
            return HttpResponse::InternalServerError().finish();
        },
    };

    match db.get_my_invited_users(&my_invite_code) {
        Ok(invited_users) => {
            println!("API Success: /api/user/my_teams - 用户 {} 已获取 {} 个团队成员。", user_id, invited_users.len());
            HttpResponse::Ok().json(invited_users)
        },
        Err(e) => {
            eprintln!("API Error: /api/user/my_teams - 获取用户 {} 团队信息失败: {:?}", user_id, e);
            HttpResponse::InternalServerError().finish()
        }
    }
}

// 获取佣金发放记录
#[get("/commission_records")]
pub async fn get_commission_records(
    db: web::Data<Database>,
    jwt_config: web::Data<JwtConfig>,
    req: HttpRequest,
) -> impl Responder {
    println!("API Call: /api/user/commission_records received.");

    let user_id = match get_user_id_from_token(&req, &jwt_config) {
        Ok(id) => id,
        Err(resp) => {
            eprintln!("API Error: /api/user/commission_records - 未授权访问。");
            return resp;
        },
    };
    println!("API Info: /api/user/commission_records - 用户ID {} 请求佣金记录。", user_id);

    match db.get_commission_records(user_id) {
        Ok(records) => {
            println!("API Success: /api/user/commission_records - 用户 {} 已获取 {} 条佣金记录。", user_id, records.len());
            HttpResponse::Ok().json(records)
        },
        Err(e) => {
            eprintln!("API Error: /api/user/commission_records - 获取用户 {} 佣金记录失败: {:?}", user_id, e);
            HttpResponse::InternalServerError().finish()
        }
    }
}

// 新增: 获取用户自己的提现记录
#[get("/withdrawal_records")]
pub async fn get_user_withdrawal_records(
    db: web::Data<Database>,
    jwt_config: web::Data<JwtConfig>,
    req: HttpRequest,
) -> impl Responder {
    println!("API Call: /api/user/withdrawal_records received.");

    let user_id = match get_user_id_from_token(&req, &jwt_config) {
        Ok(id) => id,
        Err(resp) => {
            eprintln!("API Error: /api/user/withdrawal_records - 未授权访问。");
            return resp;
        },
    };
    println!("API Info: /api/user/withdrawal_records - 用户ID {} 请求提现记录。", user_id);

    match db.get_user_withdrawal_orders(user_id) {
        Ok(records) => {
            println!("API Success: /api/user/withdrawal_records - 用户 {} 已获取 {} 条提现记录。", user_id, records.len());
            HttpResponse::Ok().json(records)
        },
        Err(e) => {
            eprintln!("API Error: /api/user/withdrawal_records - 获取用户 {} 提现记录失败: {:?}", user_id, e);
            HttpResponse::InternalServerError().finish()
        }
    }
}


// 绑定用户自己的 BSC 地址 (新增)
#[post("/bind_bsc_address")]
pub async fn bind_bsc_address(
    db: web::Data<Database>,
    jwt_config: web::Data<JwtConfig>,
    req: HttpRequest,
    bind_req: web::Json<BindBscAddressRequest>,
) -> impl Responder {
    println!("API Call: /api/user/bind_bsc_address received.");

    let user_id = match get_user_id_from_token(&req, &jwt_config) {
        Ok(id) => id,
        Err(resp) => {
            eprintln!("API Error: /api/user/bind_bsc_address - 未授权访问。");
            return resp;
        },
    };
    println!("API Info: /api/user/bind_bsc_address - 用户ID {} 尝试绑定 BSC 地址: {}", user_id, bind_req.bsc_address);

    if !is_valid_evm_address(&bind_req.bsc_address) {
        eprintln!("API Error: /api/user/bind_bsc_address - 用户 {} 提供的 BSC 地址格式无效: {}", user_id, bind_req.bsc_address);
        return HttpResponse::BadRequest().json(
            serde_json::json!({"error": "无效的 BSC 地址格式"})
        );
    }

    let bound_at = get_current_utc_time_string();

    match db.bind_user_bsc_address(user_id, &bind_req.bsc_address, &bound_at) {
        Ok(_) => {
            println!("API Success: /api/user/bind_bsc_address - 用户 {} 成功绑定 BSC 地址: {}", user_id, bind_req.bsc_address);
            HttpResponse::Ok().json(serde_json::json!({"message": "BSC 地址绑定成功"}))
        },
        Err(e) => {
            eprintln!("API Error: /api/user/bind_bsc_address - 用户 {} 绑定 BSC 地址失败: {:?}", user_id, e);
            HttpResponse::InternalServerError().json(serde_json::json!({"error": "BSC 地址绑定失败"}))
        },
    }
}

// 获取当前是否有 DAO 拍卖正在进行 (公开，无需 JWT) (新增)
#[get("/current_dao_auction")]
pub async fn get_current_dao_auction(
    db: web::Data<Database>,
) -> impl Responder {
    println!("API Call: /api/user/current_dao_auction received.");

    match db.get_current_dao_auction() {
        Ok(Some(auction)) => {
            println!("API Success: /api/user/current_dao_auction - 正在进行 DAO 拍卖。");
            HttpResponse::Ok().json(CurrentDaoAuctionResponse {
                is_auction_in_progress: true,
                start_time: Some(auction.start_time),
                end_time: Some(auction.end_time),
                admin_bsc_address: Some(auction.admin_bsc_address),
            })
        },
        Ok(None) => {
            println!("API Info: /api/user/current_dao_auction - 没有正在进行的 DAO 拍卖。");
            HttpResponse::Ok().json(CurrentDaoAuctionResponse {
                is_auction_in_progress: false,
                start_time: None,
                end_time: None,
                admin_bsc_address: None,
            })
        },
        Err(e) => {
            eprintln!("API Error: /api/user/current_dao_auction - 获取当前 DAO 拍卖信息失败: {:?}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({"error": "获取 DAO 拍卖信息失败"}))
        },
    }
}

// 新增：获取所有学院文章列表 (公开)
#[get("/academy/articles")]
pub async fn get_articles(
    db: web::Data<Database>,
) -> impl Responder {
    println!("API Call: /api/user/academy/articles - 收到获取文章列表请求。");

    match db.get_all_academy_articles(true) { // 只获取 is_displayed 为 true 的文章
        Ok(articles) => {
            println!("API Success: /api/user/academy/articles - 已获取 {} 篇文章摘要。", articles.len());
            HttpResponse::Ok().json(articles) // 返回 AcademyArticleSummary 列表
        },
        Err(e) => {
            eprintln!("API Error: /api/user/academy/articles - 获取文章列表失败: {:?}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({"error": "获取文章列表失败"}))
        },
    }
}

// 新增：根据 ID 获取学院文章详情 (公开)
#[get("/academy/articles/{id}")]
pub async fn get_article_detail(
    db: web::Data<Database>,
    path: web::Path<i64>,
) -> impl Responder {
    let article_id = path.into_inner();
    println!("API Call: /api/user/academy/articles/{} - 收到获取文章详情请求。", article_id);

    match db.get_academy_article_by_id(article_id) {
        Ok(Some(article)) => {
            // 只有当 is_displayed 为 true 时才返回文章内容，否则返回 404
            if article.is_displayed {
                println!("API Success: /api/user/academy/articles/{} - 已获取文章详情。", article_id);
                HttpResponse::Ok().json(article)
            } else {
                eprintln!("API Error: /api/user/academy/articles/{} - 文章未显示，无法访问。", article_id);
                HttpResponse::NotFound().json(serde_json::json!({"error": "文章未找到或不可用"}))
            }
        },
        Ok(None) => {
            eprintln!("API Error: /api/user/academy/articles/{} - 未找到文章。", article_id);
            HttpResponse::NotFound().json(serde_json::json!({"error": "文章未找到"}))
        },
        Err(e) => {
            eprintln!("API Error: /api/user/academy/articles/{} - 获取文章详情失败: {:?}", article_id, e);
            HttpResponse::InternalServerError().json(serde_json::json!({"error": "获取文章详情失败"}))
        },
    }
}


// 用户修改昵称
#[put("/nickname")]
pub async fn update_user_nickname(
    db: web::Data<Database>,
    jwt_config: web::Data<JwtConfig>,
    req: HttpRequest,
    update_req: web::Json<UpdateNicknameRequest>,
) -> impl Responder {
    println!("API Call: /api/user/profile/nickname - 收到用户修改昵称请求。");

    let user_id = match get_user_id_from_token(&req, &jwt_config) {
        Ok(id) => id,
        Err(resp) => {
            eprintln!("API Error: /api/user/profile/nickname - 未授权访问。");
            return resp;
        },
    };

    let new_nickname = &update_req.nickname;

    if new_nickname.trim().is_empty() {
        eprintln!("API Error: /api/user/profile/nickname - 昵称不能为空。");
        return HttpResponse::BadRequest().json(serde_json::json!({"error": "昵称不能为空"}));
    }

    match db.update_user_nickname(user_id, new_nickname) {
        Ok(_) => {
            println!("API Success: /api/user/profile/nickname - 用户 {} 的昵称已更新为 '{}'。", user_id, new_nickname);
            HttpResponse::Ok().json(serde_json::json!({"message": "昵称更新成功"}))
        },
        Err(e) => {
            eprintln!("API Error: /api/user/profile/nickname - 更新用户 {} 昵称失败: {:?}", user_id, e);
            HttpResponse::InternalServerError().json(serde_json::json!({"error": "更新昵称失败"}))
        },
    }
}