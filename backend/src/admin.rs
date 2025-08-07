// GNTX 数据库操作底层函数，供 gntx_sync 调用
use crate::db::UserGNTXInfo;
use anyhow::anyhow;

/// 获取所有用户 GNTX 信息（底层函数，非 handler）
pub fn db_get_all_user_gntx_info(db: &Database) -> Result<Vec<UserGNTXInfo>, anyhow::Error> {
    db.get_all_user_bsc_addresses_with_gntx().map_err(|e| anyhow::anyhow!(e))
}

/// 通过邮箱更新 GNTX 余额（底层函数，非 handler）
pub fn db_update_user_gntx_balance(db: &Database, email: &str, gntx_balance: f64) -> Result<(), anyhow::Error> {
    if !crate::utils::is_valid_email(email) {
        return Err(anyhow::anyhow!("邮箱格式不正确"));
    }
    if gntx_balance < 0.0 {
        return Err(anyhow::anyhow!("GNTX 数量不能为负数"));
    }
    db.update_user_gntx_balance_by_email(email, gntx_balance).map_err(|e| anyhow::anyhow!(e))
}
// src/admin.rs
use actix_web::{get, post, delete, web, HttpResponse, Responder,put}; 
use serde::{Deserialize, Serialize};
use crate::db::Database;
use crate::utils::{is_valid_date, get_current_utc_time_string, is_valid_evm_address, is_valid_email, is_valid_password, hash_password, generate_invite_code}; // 引入更多 utils 函数


#[derive(Deserialize)]
pub struct UpdateNtxControlRequest {
    pub admin_fee_percentage: f64,
}

#[derive(Deserialize)]
pub struct UpdateGntxBalanceRequest {
    pub email: String,
    pub gntx_balance: f64,
}

#[derive(Deserialize)]
pub struct CreateUserByAdminRequest {
    pub email: String,
    pub nickname: String,
    pub password: String,
    pub invite_code: Option<String>,
    pub is_admin: Option<bool>,
}


#[derive(Deserialize)]
pub struct CreateExchangeRequest {
    pub name: String,
    #[serde(rename = "logoUrl")]
    pub logo_url: String,
    #[serde(rename = "miningEfficiency")]
    pub mining_efficiency: f64,
    #[serde(rename = "cexUrl")]
    pub cex_url: String,
}

// 新增：更新交易所请求体
#[derive(Deserialize)]
pub struct UpdateExchangeRequest {
    pub id: i64,
    pub name: String,
    #[serde(rename = "logoUrl")]
    pub logo_url: String,
    #[serde(rename = "miningEfficiency")]
    pub mining_efficiency: f64,
    #[serde(rename = "cexUrl")]
    pub cex_url: String,
}

// 新增：删除交易所请求体
#[derive(Deserialize)]
pub struct DeleteExchangeRequest {
    pub id: i64,
}

// 新增：创建文章请求体 (已存在)
#[derive(Deserialize)]
pub struct CreateArticleRequest {
    pub title: String,
    pub summary: String,
    #[serde(rename = "imageUrl")]
    pub image_url: Option<String>,
    #[serde(rename = "isDisplayed")]
    pub is_displayed: bool,
    pub content: String, // Markdown 格式
}

// 新增：更新文章请求体 (已存在)
#[derive(Deserialize)]
pub struct UpdateArticleRequest {
    pub title: String,
    pub summary: String,
    #[serde(rename = "imageUrl")]
    pub image_url: Option<String>,
    #[serde(rename = "isDisplayed")]
    pub is_displayed: bool,
    pub content: String, // Markdown 格式
}

// 新增：删除文章请求体
#[derive(Deserialize)]
pub struct DeleteArticleRequest {
    pub id: i64,
}


#[derive(Deserialize)]
pub struct AddDailyTradeDataRequest {
    // 可选，但至少一个必须存在
    pub user_id: Option<i64>,
    pub exchange_uid: Option<String>, // UID 通常是字符串形式

    pub exchange_id: i64,
    pub trade_volume_usdt: f64,
    pub fee_usdt: f64,
    pub trade_date: String,
}

#[derive(Deserialize)]
pub struct UpdateExchangeMiningEfficiencyRequest {
    pub exchange_id: i64,
    pub new_efficiency: f64,
}

#[derive(Deserialize)]
pub struct ToggleUserStatusRequest {
    pub user_id: i64,
    pub is_active: bool, // true 为激活，false 为封禁
}

#[derive(Debug, Deserialize, Serialize)]
pub struct AdminWithdrawalOrderResponse {
    pub id: i64,
    pub user_id: i64,
    pub user_email: String,
    pub amount: f64,
    pub currency: String,
    pub to_address: String,
    pub is_confirmed: bool,
    pub created_at: String,
    pub processed_at: Option<String>,
    pub status: String,
}

#[derive(Deserialize)]
pub struct UpdateWithdrawalStatusRequest {
    pub order_id: i64,
    pub status: String, // "approved" 或 "rejected"
}

#[derive(Deserialize)]
pub struct UpdateUserTotalDataRequest {
    pub user_id: i64,
    pub total_mining: f64,
    pub total_trading_cost: f64,
}

#[derive(Deserialize)]
pub struct UpdateDailyUserDataRequest {
    pub user_id: i64,
    pub date: String,
    pub mining_output: f64,
    pub total_trading_cost: f64,
}

#[derive(Deserialize)]
pub struct UpdatePlatformTotalDataRequest {
    pub total_mined: f64,
    pub total_commission: f64,
    pub total_burned: f64,
    pub total_trading_volume: f64,
    pub platform_users: i64,
}

#[derive(Deserialize)]
pub struct UpdateDailyPlatformDataRequest {
    pub date: String,
    pub mining_output: f64,
    pub burned: f64,
    pub commission: f64,
    pub trading_volume: f64,
    pub miners: i64,
}

#[derive(Deserialize)]
pub struct UpdateUserProfileRequest {
    pub user_id: i64,
    pub nickname: String,
    pub email: String,
    #[serde(rename = "myInviteCode")]
    pub my_invite_code: String,
    pub exp: i64,
    #[serde(rename = "usdtBalance")]
    pub usdt_balance: f64,
    #[serde(rename = "ntxBalance")]
    pub ntx_balance: f64,
    #[serde(rename = "isActive")]
    pub is_active: bool,
    #[serde(rename = "isAdmin")]
    pub is_admin: bool,
    #[serde(rename = "isBroker")]
    pub is_broker: bool, // 是否为强制为经纪商（为true时系统强制判定为经纪商）

    pub password: Option<String>,
}

#[derive(Deserialize)]
pub struct StartDaoAuctionRequest {
    #[serde(rename = "adminBscAddress")]
    pub admin_bsc_address: String,
    #[serde(rename = "startTime")]
    pub start_time: String, // ISO 8601 格式
    #[serde(rename = "durationMinutes")]
    pub duration_minutes: i64, // 拍卖持续时间（分钟）
}

#[derive(Deserialize)]
pub struct DateRangeRequest {
    #[serde(rename = "startDate")]
    pub start_date: String,
    #[serde(rename = "endDate")]
    pub end_date: String,
}

#[derive(Deserialize)]
pub struct DateQueryRequest {
    pub date: String,
}

// 获取管理员仪表盘数据
#[get("/dashboard")]
pub async fn get_dashboard_data(db: web::Data<Database>) -> impl Responder {
    println!("API Info: /api/admin/dashboard - 收到获取仪表盘数据的请求。");
    match db.get_admin_dashboard_data() {
        Ok(data) => {
            println!("API Success: /api/admin/dashboard - 成功获取仪表盘数据。");
            HttpResponse::Ok().json(data)
        }
        Err(e) => {
            eprintln!("API Error: /api/admin/dashboard - 获取仪表盘数据失败: {:?}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({"error": "获取仪表盘数据失败"}))
        }
    }
}


// 获取所有用户信息
#[get("/users")]
pub async fn get_all_users(db: web::Data<Database>) -> impl Responder {
    println!("API Info: /api/admin/users - 收到获取所有用户信息的请求。");
    match db.get_all_users() {
        Ok(users) => {
            println!("API Success: /api/admin/users - 已获取 {} 个用户信息。", users.len());
            HttpResponse::Ok().json(users)
        },
        Err(e) => {
            eprintln!("API Error: /api/admin/users - 获取所有用户信息失败: {:?}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({"error": "获取所有用户信息失败"}))
        },
    }
}

//管理员直接添加新用户
#[post("/users")]
pub async fn add_user_by_admin(
    db: web::Data<Database>,
    req: web::Json<CreateUserByAdminRequest>,
) -> impl Responder {
    println!("API Info: /api/admin/users - 收到管理员添加用户请求。Email: {}", req.email);

    // 验证邮箱和密码格式
    if !is_valid_email(&req.email) {
        eprintln!("API Error: /api/admin/users - 添加用户失败: 无效的邮箱格式: {}", req.email);
        return HttpResponse::BadRequest().json(serde_json::json!({"error": "无效的邮箱格式"}));
    }
    if !is_valid_password(&req.password) {
        eprintln!("API Error: /api/admin/users - 添加用户失败: 密码不符合要求。");
        return HttpResponse::BadRequest().json(serde_json::json!({"error": "密码必须为8-32个字符且包含一个大写字母"}));
    }

    // 检查用户是否已存在
    if db.get_user_by_email(&req.email).unwrap_or(None).is_some() {
        eprintln!("API Error: /api/admin/users - 添加用户失败: 邮箱已被注册: {}", req.email);
        return HttpResponse::Conflict().json(serde_json::json!({"error": "邮箱已被注册"}));
    }

    // 处理邀请码
    let mut inviter_email: Option<String> = None;
    if let Some(ref invite_code) = req.invite_code {
        match db.get_email_by_invite_code(invite_code) {
            Ok(Some(email)) => inviter_email = Some(email),
            Ok(None) => {
                eprintln!("API Error: /api/admin/users - 添加用户失败: 邀请码无效或不存在: {}", invite_code);
                return HttpResponse::BadRequest().json(serde_json::json!({"error": "邀请码无效或不存在"}));
            },
            Err(e) => {
                eprintln!("API Error: /api/admin/users - 添加用户失败: 获取邀请码信息失败: {:?}", e);
                return HttpResponse::InternalServerError().json(serde_json::json!({"error": "获取邀请码信息失败"}));
            },
        }
    }

    // 哈希密码
    let hashed_password = match hash_password(&req.password) {
        Ok(h) => h,
        Err(e) => {
            eprintln!("API Error: /api/admin/users - 添加用户失败: 密码哈希失败: {:?}", e);
            return HttpResponse::InternalServerError().json(serde_json::json!({"error": "密码哈希失败"}));
        },
    };

    // 生成用户自己的邀请码
    let user_invite_code = generate_invite_code();

    // 确定是否为管理员用户
    let is_admin_user = req.is_admin.unwrap_or(false);

    // 启动数据库事务
    let conn_mutex = db.conn.clone();
    let mut conn = conn_mutex.lock().unwrap();
    let tx = match conn.transaction() {
        Ok(t) => t,
        Err(e) => {
            eprintln!("API Error: /api/admin/users - 添加用户失败: 开启事务失败: {:?}", e);
            return HttpResponse::InternalServerError().json(serde_json::json!({"error": "数据库事务开启失败"}));
        },
    };

    match db.create_user(
        &req.email,
        &req.nickname,
        &hashed_password,
        &user_invite_code,
        inviter_email.as_deref(),
        is_admin_user, // 传入 is_admin 参数
        &tx
    ) {
        Ok(new_user_id) => {
            // 如果提供了邀请码且是管理员邀请码，则标记其已使用 (此处逻辑与 auth.rs 的注册逻辑保持一致)
            // if req.invite_code.as_deref() == Some("NTXADMIN") { // 假设 NTXADMIN 是管理员邀请码
            //     if let Err(e) = db.use_special_invite_code("NTXADMIN", new_user_id, &tx) {
            //         eprintln!("API Error: /api/admin/users - 标记管理员邀请码使用失败: {:?}", e);
            //         let _ = tx.rollback();
            //         return HttpResponse::InternalServerError().json(serde_json::json!({"error": "标记管理员邀请码使用失败"}));
            //     }
            // }

            match tx.commit() {
                Ok(_) => {
                    println!("API Success: /api/admin/users - 管理员成功添加用户，ID: {}", new_user_id);
                    HttpResponse::Created().json(serde_json::json!({"message": "用户添加成功", "userId": new_user_id}))
                },
                Err(e) => {
                    eprintln!("API Error: /api/admin/users - 管理员添加用户事务提交失败: {:?}", e);
                    HttpResponse::InternalServerError().json(serde_json::json!({"error": "用户添加失败，事务提交失败"}))
                },
            }
        },
        Err(e) => {
            let _ = tx.rollback(); // 发生错误时回滚事务
            eprintln!("API Error: /api/admin/users - 管理员添加用户失败: {:?}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({"error": "用户添加失败"}))
        },
    }
}


// 获取单个用户完整信息
#[get("/users/{user_id}/full_info")]
pub async fn get_user_full_info(
    db: web::Data<Database>,
    path: web::Path<i64>,
) -> impl Responder {
    let user_id = path.into_inner();
    println!("API Info: /api/admin/users/{}/full_info - 收到获取用户完整信息的请求。", user_id);
    match db.get_user_info_full(user_id) {
        Ok(Some(user_info)) => {
            println!("API Success: /api/admin/users/{}/full_info - 已获取用户完整信息。", user_id);
            HttpResponse::Ok().json(user_info)
        },
        Ok(None) => {
            // 修复：确保为每个占位符提供参数
            eprintln!("API Error: /api/admin/users/{}/full_info - 未找到用户ID {}。", user_id, user_id);
            HttpResponse::NotFound().json(serde_json::json!({"error": "未找到该用户"}))
        },
        Err(e) => {
            eprintln!("API Error: /api/admin/users/{}/full_info - 获取用户完整信息失败: {:?}", user_id, e);
            HttpResponse::InternalServerError().json(serde_json::json!({"error": "获取用户完整信息失败"}))
        },
    }
}

//管理员删除用户
#[delete("/users/{user_id}")]
pub async fn delete_user_by_admin(
    db: web::Data<Database>,
    path: web::Path<i64>,
) -> impl Responder {
    let user_id = path.into_inner();
    println!("API Info: /api/admin/users/{} - 收到删除用户请求。", user_id);

    match db.delete_user(user_id) {
        Ok(_) => {
            println!("API Success: /api/admin/users/{} - 用户删除成功。", user_id);
            HttpResponse::Ok().json(serde_json::json!({"message": "用户删除成功"}))
        },
        Err(e) => {
            eprintln!("API Error: /api/admin/users/{} - 删除用户失败: {:?}", user_id, e);
            HttpResponse::InternalServerError().json(serde_json::json!({"error": "删除用户失败"}))
        },
    }
}


// 获取指定用户绑定的交易所
#[get("/user/{user_id}/exchanges")]
pub async fn get_user_bound_exchanges(
    db: web::Data<Database>,
    path: web::Path<i64>,
) -> impl Responder {
    let user_id = path.into_inner();
    println!("API Info: /api/admin/user/{}/exchanges - 收到请求。", user_id);
    match db.get_user_exchanges(user_id) {
        Ok(exchanges) => {
            println!("API Success: /api/admin/user/{}/exchanges - 已获取 {} 个交易所信息。", user_id, exchanges.len());
            HttpResponse::Ok().json(exchanges)
        },
        Err(e) => {
            eprintln!("API Error: /api/admin/user/{}/exchanges - 获取用户交易所失败: {:?}", user_id, e);
            HttpResponse::InternalServerError().json(serde_json::json!({"error": "获取用户交易所失败"}))
        },
    }
}

// 获取所有交易所信息
#[get("/exchanges/all")]
pub async fn get_all_exchanges_admin(
    db: web::Data<Database>,
) -> impl Responder {
    println!("API Info: /api/admin/exchanges/all - 收到获取所有交易所请求。");
    match db.get_exchanges() {
        Ok(exchanges) => {
            println!("API Success: /api/admin/exchanges/all - 已获取 {} 个交易所信息。", exchanges.len());
            HttpResponse::Ok().json(exchanges)
        },
        Err(e) => {
            eprintln!("API Error: /api/admin/exchanges/all - 获取所有交易所信息失败: {:?}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({"error": "获取所有交易所信息失败"}))
        },
    }
}


// 创建交易所
#[post("/exchanges")]
pub async fn create_exchange(
    db: web::Data<Database>,
    req: web::Json<CreateExchangeRequest>,
) -> impl Responder {
    println!("API Info: /api/admin/exchanges - 收到创建交易所请求。名称: {}", req.name);
    match db.create_exchange(&req.name, &req.logo_url, req.mining_efficiency, &req.cex_url) {
        Ok(exchange_id) => {
            println!("API Success: /api/admin/exchanges - 交易所创建成功，ID: {}", exchange_id);
            HttpResponse::Created().json(serde_json::json!({"message": "交易所创建成功", "id": exchange_id}))
        },
        Err(e) => {
            eprintln!("API Error: /api/admin/exchanges - 创建交易所失败: {:?}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({"error": "创建交易所失败"}))
        },
    }
}

// 更新交易所信息
#[post("/exchanges/{id}")]
pub async fn update_exchange(
    db: web::Data<Database>,
    path: web::Path<i64>,
    req: web::Json<UpdateExchangeRequest>,
) -> impl Responder {
    let exchange_id = path.into_inner();
    println!("API Info: /api/admin/exchanges/{} - 收到更新交易所请求。名称: {}", exchange_id, req.name);

    if exchange_id != req.id {
        eprintln!("API Error: /api/admin/exchanges/{} - URL中的ID与请求体中的ID不匹配。", exchange_id);
        return HttpResponse::BadRequest().json(serde_json::json!({"error": "URL中的ID与请求体中的ID不匹配"}));
    }

    match db.update_exchange(req.id, &req.name, &req.logo_url, req.mining_efficiency, &req.cex_url) {
        Ok(_) => {
            println!("API Success: /api/admin/exchanges/{} - 交易所信息更新成功。", exchange_id);
            HttpResponse::Ok().json(serde_json::json!({"message": "交易所信息更新成功"}))
        },
        Err(e) => {
            eprintln!("API Error: /api/admin/exchanges/{} - 更新交易所信息失败: {:?}", exchange_id, e);
            HttpResponse::InternalServerError().json(serde_json::json!({"error": "更新交易所信息失败"}))
        },
    }
}

// 删除交易所
#[delete("/exchanges/{id}")]
pub async fn delete_exchange(
    db: web::Data<Database>,
    path: web::Path<i64>,
) -> impl Responder {
    let exchange_id = path.into_inner();
    println!("API Info: /api/admin/exchanges/{} - 收到删除交易所请求。", exchange_id);
    match db.delete_exchange(exchange_id) {
        Ok(_) => {
            println!("API Success: /api/admin/exchanges/{} - 交易所删除成功。", exchange_id);
            HttpResponse::Ok().json(serde_json::json!({"message": "交易所删除成功"}))
        },
        Err(e) => {
            eprintln!("API Error: /api/admin/exchanges/{} - 删除交易所失败: {:?}", exchange_id, e);
            HttpResponse::InternalServerError().json(serde_json::json!({"error": "删除交易所失败"}))
        },
    }
}

// 添加用户每日交易数据
#[post("/add_daily_trade_data")]
pub async fn add_daily_trade_data(
    db: web::Data<Database>,
    req: web::Json<AddDailyTradeDataRequest>,
) -> impl Responder {
    // 1. 输入验证：必须提供 user_id 或 exchange_uid
    if req.user_id.is_none() && req.exchange_uid.is_none() {
        eprintln!("API Error: /api/admin/add_daily_trade_data - 必须提供 user_id 或 exchange_uid。");
        return HttpResponse::BadRequest().json(
            serde_json::json!({"error": "必须提供 user_id 或 exchange_uid"})
        );
    }

    // 验证日期格式
    if !is_valid_date(&req.trade_date) {
        eprintln!("API Error: /api/admin/add_daily_trade_data - 无效的日期格式: {}", req.trade_date);
        return HttpResponse::BadRequest().json(
            serde_json::json!({"error": "无效的日期格式，应为YYYY-MM-DD"})
        );
    }

    // 2. 确定用户 ID
    let user_id = match req.user_id {
        Some(id) => id,
        None => {
            // 如果 user_id 不存在，则 exchange_uid 必须存在
            let exchange_uid = req.exchange_uid.as_ref().unwrap(); // 因上面的验证，这里是安全的
            match db.get_user_id_by_exchange_uid(req.exchange_id, exchange_uid) {
                Ok(Some(id)) => {
                    println!("API Info: /api/admin/add_daily_trade_data - 通过 Exchange UID '{}' 和 Exchange ID {} 查找到 User ID {}。", exchange_uid, req.exchange_id, id);
                    id
                },
                Ok(None) => {
                    eprintln!("API Error: /api/admin/add_daily_trade_data - 未找到与 Exchange UID '{}' 和 Exchange ID {} 绑定的用户。", exchange_uid, req.exchange_id);
                    return HttpResponse::NotFound().json(
                        serde_json::json!({"error": "未找到与提供的 exchange_uid 和 exchange_id 绑定的用户"})
                    );
                },
                Err(e) => {
                    eprintln!("API Error: /api/admin/add_daily_trade_data - 通过UID查询用户ID失败: {:?}", e);
                    return HttpResponse::InternalServerError().json(
                        serde_json::json!({"error": "数据库查询失败"})
                    );
                }
            }
        }
    };
    
    println!("API Info: /api/admin/add_daily_trade_data - 正在为用户 {} 添加交易数据。", user_id);

    // 3. 获取用户和交易所的附加信息 (复用现有逻辑)
    let user_email = match db.get_user_email_by_id(user_id) {
        Ok(Some(email)) => email,
        Ok(None) => {
            eprintln!("API Error: /api/admin/add_daily_trade_data - 未找到用户ID {}。", user_id);
            return HttpResponse::BadRequest().json(
                serde_json::json!({"error": format!("用户ID {} 不存在", user_id)})
            );
        },
        Err(e) => {
            eprintln!("API Error: /api/admin/add_daily_trade_data - 获取用户 {} 的邮箱失败: {:?}", user_id, e);
            return HttpResponse::InternalServerError().json(serde_json::json!({"error": "获取用户邮箱失败"}));
        },
    };

    let exchange_name = match db.get_exchange_name_by_id(req.exchange_id) {
        Ok(Some(name)) => name,
        Ok(None) => {
            eprintln!("API Error: /api/admin/add_daily_trade_data - 未找到交易所ID {}。", req.exchange_id);
            return HttpResponse::BadRequest().json(
                serde_json::json!({"error": "交易所ID不存在"})
            );
        },
        Err(e) => {
            eprintln!("API Error: /api/admin/add_daily_trade_data - 获取交易所 {} 的名称失败: {:?}", req.exchange_id, e);
            return HttpResponse::InternalServerError().json(serde_json::json!({"error": "获取交易所名称失败"}));
        },
    };

    // 4. 调用数据库函数添加或更新交易数据
    if let Err(e) = db.add_or_update_daily_trade_data(
        user_id,
        user_email,
        req.exchange_id,
        exchange_name,
        req.trade_volume_usdt,
        req.fee_usdt,
        &req.trade_date,
    ) {
        eprintln!("API Error: /api/admin/add_daily_trade_data - 添加用户 {} 的每日交易数据失败: {:?}", user_id, e);
        return HttpResponse::InternalServerError().json(serde_json::json!({"error": "添加每日交易数据失败"}));
    }

    println!("API Success: /api/admin/add_daily_trade_data - 成功添加/更新用户 {} 的每日交易数据。", user_id);
    HttpResponse::Ok().json(serde_json::json!({"message": "每日交易数据添加/更新成功"}))
}

// 获取指定日期的所有用户交易记录
#[get("/daily_trades")]
pub async fn get_daily_trades_admin(
    db: web::Data<Database>,
    query: web::Query<DateQueryRequest>,
) -> impl Responder {
    let date_str = query.date.clone();
    println!("API Info: /api/admin/daily_trades - 收到获取日期 {} 的所有用户交易记录请求。", date_str);

    if !is_valid_date(&date_str) {
        eprintln!("API Error: /api/admin/daily_trades - 无效的日期格式: {}", date_str);
        return HttpResponse::BadRequest().json(
            serde_json::json!({"error": "无效的日期格式，应为YYYY-MM-DD"})
        );
    }

    match db.get_all_daily_user_trades_for_date(&date_str) {
        Ok(records) => {
            println!("API Success: /api/admin/daily_trades - 已获取 {} 条日期 {} 的交易记录。", records.len(), date_str);
            HttpResponse::Ok().json(records)
        },
        Err(e) => {
            eprintln!("API Error: /api/admin/daily_trades - 获取日期 {} 的交易记录失败: {:?}", date_str, e);
            HttpResponse::InternalServerError().json(serde_json::json!({"error": "获取每日交易记录失败"}))
        },
    }
}


// 修改交易所挖矿效率
#[post("/update_exchange_mining_efficiency")]
pub async fn update_exchange_mining_efficiency(
    db: web::Data<Database>,
    req: web::Json<UpdateExchangeMiningEfficiencyRequest>,
) -> impl Responder {
    println!("API Info: /api/admin/update_exchange_mining_efficiency - 收到交易所 {} 的请求。", req.exchange_id);
    match db.update_exchange_mining_efficiency(req.exchange_id, req.new_efficiency) {
        Ok(_) => {
            println!("API Success: /api/admin/update_exchange_mining_efficiency - 成功更新交易所 {} 的挖矿效率。", req.exchange_id);
            HttpResponse::Ok().json(serde_json::json!({"message": "更新交易所挖矿效率成功"}))
        },
        Err(e) => {
            eprintln!("API Error: /api/admin/update_exchange_mining_efficiency - 更新交易所 {} 挖矿效率失败: {:?}", req.exchange_id, e);
            HttpResponse::InternalServerError().json(serde_json::json!({"error": "更新交易所挖矿效率失败"}))
        },
    }
}

// 封禁/解封用户
#[post("/toggle_user_status")]
pub async fn toggle_user_status(
    db: web::Data<Database>,
    req: web::Json<ToggleUserStatusRequest>,
) -> impl Responder {
    println!("API Info: /api/admin/toggle_user_status - 收到用户 {} 的请求。状态: {}", req.user_id, req.is_active);
    match db.update_user_active_status(req.user_id, req.is_active) {
        Ok(_) => {
            println!("API Success: /api/admin/toggle_user_status - 成功更新用户 {} 状态为 {}。", req.user_id, req.is_active);
            HttpResponse::Ok().json(serde_json::json!({"message": "用户状态更新成功"}))
        },
        Err(e) => {
            eprintln!("API Error: /api/admin/toggle_user_status - 更新用户 {} 状态失败: {:?}", req.user_id, e);
            HttpResponse::InternalServerError().json(serde_json::json!({"error": "更新用户状态失败"}))
        },
    }
}

// 获取所有提现订单
#[get("/withdrawal_orders")]
pub async fn get_all_withdrawal_orders(db: web::Data<Database>) -> impl Responder {
    println!("API Info: /api/admin/withdrawal_orders - 收到获取所有提现订单的请求。");
    match db.get_all_withdrawal_orders() {
        Ok(orders) => {
            println!("API Success: /api/admin/withdrawal_orders - 成功获取所有提现订单。");
            HttpResponse::Ok().json(orders)
        },
        Err(e) => {
            eprintln!("API Error: /api/admin/withdrawal_orders - 获取提现订单失败: {:?}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({"error": "获取提现订单失败"}))
        },
    }
}

// 确认/拒绝提现订单
#[post("/withdrawal_orders/update_status")]
pub async fn update_withdrawal_order_status(
    db: web::Data<Database>,
    req: web::Json<UpdateWithdrawalStatusRequest>,
) -> impl Responder {
    println!("API Info: /api/admin/withdrawal_orders/update_status - 收到订单 {} 的状态更新请求，状态: {}", req.order_id, req.status);

    if !["approved", "rejected"].contains(&req.status.as_str()) {
        eprintln!("API Error: /api/admin/withdrawal_orders/update_status - 无效的状态: {}", req.status);
        return HttpResponse::BadRequest().json(serde_json::json!({"error": "无效的订单状态，只能是 'approved' 或 'rejected'"}));
    }

    let processed_at = get_current_utc_time_string(); // 获取当前 UTC 时间

    match db.update_withdrawal_order_status(req.order_id, &req.status, &processed_at) {
        Ok(_) => {
            println!("API Success: /api/admin/withdrawal_orders/update_status - 成功更新订单 {} 状态为 {}。", req.order_id, req.status);
            HttpResponse::Ok().json(serde_json::json!({"message": format!("提现订单 {} 已被标记为 {}", req.order_id, req.status)}))
        },
        Err(e) => {
            eprintln!("API Error: /api/admin/withdrawal_orders/update_status - 更新订单 {} 状态失败: {:?}", req.order_id, e);
            HttpResponse::InternalServerError().json(serde_json::json!({"error": "更新提现订单状态失败"}))
        },
    }
}

// 修改用户总数据 (已存在)
#[post("/user_data/update_total")]
pub async fn update_user_total_data(
    db: web::Data<Database>,
    req: web::Json<UpdateUserTotalDataRequest>,
) -> impl Responder {
    println!("API Info: /api/admin/user_data/update_total - 收到用户 {} 总数据更新请求。", req.user_id);
    match db.update_user_total_data(req.user_id, req.total_mining, req.total_trading_cost) {
        Ok(_) => {
            println!("API Success: /api/admin/user_data/update_total - 成功更新用户 {} 总数据。", req.user_id);
            HttpResponse::Ok().json(serde_json::json!({"message": "用户总数据更新成功"}))
        },
        Err(e) => {
            eprintln!("API Error: /api/admin/user_data/update_total - 更新用户 {} 总数据失败: {:?}", req.user_id, e);
            HttpResponse::InternalServerError().json(serde_json::json!({"error": "更新用户总数据失败"}))
        },
    }
}

// 修改每日用户数据 (已存在)
#[post("/user_data/update_daily")]
pub async fn update_daily_user_data(
    db: web::Data<Database>,
    req: web::Json<UpdateDailyUserDataRequest>,
) -> impl Responder {
    println!("API Info: /api/admin/user_data/update_daily - 收到用户 {} 日期 {} 每日数据更新请求。", req.user_id, req.date);

    if !is_valid_date(&req.date) {
        eprintln!("API Error: /api/admin/user_data/update_daily - 无效的日期格式: {}", req.date);
        return HttpResponse::BadRequest().json(
            serde_json::json!({"error": "无效的日期格式，应为YYYY-MM-DD"})
        );
    }

    match db.update_daily_user_data_by_admin(req.user_id, &req.date, req.mining_output, req.total_trading_cost) {
        Ok(_) => {
            println!("API Success: /api/admin/user_data/update_daily - 成功更新用户 {} 日期 {} 每日数据。", req.user_id, req.date);
            HttpResponse::Ok().json(serde_json::json!({"message": "每日用户数据更新成功"}))
        },
        Err(e) => {
            eprintln!("API Error: /api/admin/user_data/update_daily - 更新用户 {} 日期 {} 每日数据失败: {:?}", req.user_id, req.date, e);
            HttpResponse::InternalServerError().json(serde_json::json!({"error": "更新每日用户数据失败"}))
        },
    }
}

// 获取用户指定日期范围的每日数据
#[get("/users/{user_id}/daily_data/history")]
pub async fn get_user_daily_data_history(
    db: web::Data<Database>,
    path: web::Path<i64>,
    query: web::Query<DateRangeRequest>,
) -> impl Responder {
    let user_id = path.into_inner();
    let start_date = query.start_date.clone();
    let end_date = query.end_date.clone();
    // 修复：更改格式字符串，将路径参数 user_id 放在 {} 中，并确保参数数量匹配
    println!("API Info: /api/admin/users/{}/daily_data/history - 收到获取日期范围 {} 至 {} 的每日数据请求。", user_id, start_date, end_date);

    if !is_valid_date(&start_date) || !is_valid_date(&end_date) {
        eprintln!("API Error: /api/admin/users/{}/daily_data/history - 无效的日期格式: {} 或 {}", user_id, start_date, end_date);
        return HttpResponse::BadRequest().json(
            serde_json::json!({"error": "无效的日期格式，应为YYYY-MM-DD"})
        );
    }

    match db.get_daily_user_data_for_range(user_id, &start_date, &end_date) {
        Ok(data) => {
            // 修复：更改格式字符串，确保参数数量匹配
            println!("API Success: /api/admin/users/{}/daily_data/history - 已获取用户 {} 日期范围 {} 至 {} 的每日数据。", user_id, user_id, start_date, end_date);
            HttpResponse::Ok().json(data)
        },
        Err(e) => {
            // 修复：更改格式字符串，确保参数数量匹配
            eprintln!("API Error: /api/admin/users/{}/daily_data/history - 获取用户 {} 日期范围数据失败: {:?}", user_id, user_id, e);
            HttpResponse::InternalServerError().json(serde_json::json!({"error": "获取用户每日数据失败"}))
        },
    }
}


// 修改平台总数据 (已存在)
#[post("/platform_data/update_total")]
pub async fn update_platform_total_data(
    db: web::Data<Database>,
    req: web::Json<UpdatePlatformTotalDataRequest>,
) -> impl Responder {
    println!("API Info: /api/admin/platform_data/update_total - 收到平台总数据更新请求。");
    match db.update_platform_total_data(
        req.total_mined,
        req.total_commission,
        req.total_burned,
        req.total_trading_volume,
        req.platform_users,
    ) {
        Ok(_) => {
            println!("API Success: /api/admin/platform_data/update_total - 成功更新平台总数据。");
            HttpResponse::Ok().json(serde_json::json!({"message": "平台总数据更新成功"}))
        },
        Err(e) => {
            eprintln!("API Error: /api/admin/platform_data/update_total - 更新平台总数据失败: {:?}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({"error": "更新平台总数据失败"}))
        },
    }
}

// 修改每日平台数据 (已存在)
#[post("/platform_data/update_daily")]
pub async fn update_daily_platform_data(
    db: web::Data<Database>,
    req: web::Json<UpdateDailyPlatformDataRequest>,
) -> impl Responder {
    println!("API Info: /api/admin/platform_data/update_daily - 收到日期 {} 每日平台数据更新请求。", req.date);

    if !is_valid_date(&req.date) {
        eprintln!("API Error: /api/admin/platform_data/update_daily - 无效的日期格式: {}", req.date);
        return HttpResponse::BadRequest().json(
            serde_json::json!({"error": "无效的日期格式，应为YYYY-MM-DD"})
        );
    }

    match db.update_daily_platform_data_by_admin(
        &req.date,
        req.mining_output,
        req.burned,
        req.commission,
        req.trading_volume,
        req.miners,
    ) {
        Ok(_) => {
            println!("API Success: /api/admin/platform_data/update_daily - 成功更新日期 {} 每日平台数据。", req.date);
            HttpResponse::Ok().json(serde_json::json!({"message": "每日平台数据更新成功"}))
        },
        Err(e) => {
            eprintln!("API Error: /api/admin/platform_data/update_daily - 更新日期 {} 每日平台数据失败: {:?}", req.date, e);
            HttpResponse::InternalServerError().json(serde_json::json!({"error": "更新每日平台数据失败"}))
        },
    }
}

// 获取历史平台数据 (日期范围)
#[get("/platform_data/history")]
pub async fn get_platform_data_history(
    db: web::Data<Database>,
    query: web::Query<DateRangeRequest>,
) -> impl Responder {
    let start_date = query.start_date.clone();
    let end_date = query.end_date.clone();
    println!("API Info: /api/admin/platform_data/history - 收到获取日期范围 {} 至 {} 的平台历史数据请求。", start_date, end_date);

    if !is_valid_date(&start_date) || !is_valid_date(&end_date) {
        eprintln!("API Error: /api/admin/platform_data/history - 无效的日期格式: {} 或 {}", start_date, end_date);
        return HttpResponse::BadRequest().json(
            serde_json::json!({"error": "无效的日期格式，应为YYYY-MM-DD"})
        );
    }

    match db.get_historical_platform_data(&start_date, &end_date) {
        Ok(data) => {
            println!("API Success: /api/admin/platform_data/history - 已获取日期范围 {} 至 {} 的平台历史数据。", start_date, end_date);
            HttpResponse::Ok().json(data)
        },
        Err(e) => {
            eprintln!("API Error: /api/admin/platform_data/history - 获取平台历史数据失败: {:?}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({"error": "获取平台历史数据失败"}))
        },
    }
}

#[post("/user_profile/update")]
pub async fn update_user_profile(
    db: web::Data<Database>,
    req: web::Json<UpdateUserProfileRequest>,
) -> impl Responder {
    println!("API Info: /api/admin/user_profile/update - 收到用户 {} 个人信息更新请求。", req.user_id);

    // 对于密码更新，我们直接使用 req.user_id，无需先查询 email
    if let Some(ref new_password) = req.password {
        if !is_valid_password(new_password) {
            eprintln!("API Error: /api/admin/user_profile/update - 密码不符合要求。");
            return HttpResponse::BadRequest().json(serde_json::json!({"error": "密码必须为8-32个字符且包含一个大写字母"}));
        }
        let hashed_password = match hash_password(new_password) {
            Ok(h) => h,
            Err(e) => {
                eprintln!("API Error: /api/admin/user_profile/update - 密码哈希失败: {:?}", e);
                return HttpResponse::InternalServerError().json(serde_json::json!({"error": "密码哈希失败"}));
            }
        };
        match db.update_user_password_by_id(req.user_id, &hashed_password) {
            Ok(_) => {
                println!("API Success: /api/admin/user_profile/update - 用户 {} 密码更新成功。", req.user_id);
            }
            Err(e) => {
                eprintln!("API Error: /api/admin/user_profile/update - 更新用户 {} 密码失败: {:?}", req.user_id, e);
                return HttpResponse::InternalServerError().json(serde_json::json!({"error": "更新密码失败"}));
            }
        }
    }

    // 更新其他个人信息 (除了密码)
    // 这里依然需要传入 req.user_id，并且更新其他字段
    match db.update_user_profile(
        req.user_id,
        &req.nickname,
        &req.email,
        &req.my_invite_code,
        req.exp,
        req.usdt_balance,
        req.ntx_balance,
        req.is_active,
        req.is_admin,
        req.is_broker,
    ) {
        Ok(_) => {
            println!("API Success: /api/admin/user_profile/update - 成功更新用户 {} 个人信息。", req.user_id);
            HttpResponse::Ok().json(serde_json::json!({"message": "用户个人信息更新成功"}))
        },
        Err(e) => {
            eprintln!("API Error: /api/admin/user_profile/update - 更新用户 {} 个人信息失败: {:?}", req.user_id, e);
            HttpResponse::InternalServerError().json(serde_json::json!({"error": "更新用户个人信息失败"}))
        },
    }
}


// 管理员发起 DAO 拍卖
#[post("/dao_auction/start")]
pub async fn start_dao_auction(
    db: web::Data<Database>,
    req: web::Json<StartDaoAuctionRequest>,
) -> impl Responder {
    println!("API Info: /api/admin/dao_auction/start - 收到发起 DAO 拍卖请求。");

    // 验证 BSC 地址格式
    if !is_valid_evm_address(&req.admin_bsc_address) {
        eprintln!("API Error: /api/admin/dao_auction/start - 无效的管理员 BSC 收款地址: {}", req.admin_bsc_address);
        return HttpResponse::BadRequest().json(
            serde_json::json!({"error": "无效的 BSC 地址格式"})
        );
    }

    // 验证开始时间格式 (这里假设传入的是 UTC ISO 8601 格式)
    // 可以添加更严格的日期时间解析和验证
    let start_time_parsed = match chrono::DateTime::parse_from_rfc3339(&req.start_time) {
        Ok(dt) => dt.with_timezone(&chrono::Utc),
        Err(e) => {
            eprintln!("API Error: /api/admin/dao_auction/start - 开始时间格式无效: {:?}, 错误: {:?}", req.start_time, e);
            return HttpResponse::BadRequest().json(
                serde_json::json!({"error": "开始时间格式无效，应为 ISO 8601 格式 (如YYYY-MM-DDTHH:MM:SSZ)"})
            );
        }
    };

    if start_time_parsed < chrono::Utc::now() {
        eprintln!("API Error: /api/admin/dao_auction/start - 开始时间不能在当前时间之前。");
        return HttpResponse::BadRequest().json(
            serde_json::json!({"error": "开始时间不能在当前时间之前"})
        );
    }

    // 计算结束时间
    let end_time_parsed = start_time_parsed + chrono::Duration::minutes(req.duration_minutes);
    let end_time_str = end_time_parsed.to_rfc3339();

    match db.create_dao_auction(&req.admin_bsc_address, &req.start_time, &end_time_str) {
        Ok(_) => {
            println!("API Success: /api/admin/dao_auction/start - DAO 拍卖发起成功。");
            HttpResponse::Ok().json(serde_json::json!({"message": "DAO 拍卖发起成功"}))
        },
        Err(e) => {
            eprintln!("API Error: /api/admin/dao_auction/start - 发起 DAO 拍卖失败: {:?}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({"error": format!("发起 DAO 拍卖失败: {}", e)}))
        },
    }
}

// 管理员提前结束 DAO 拍卖
#[post("/dao_auction/end")]
pub async fn end_dao_auction(
    db: web::Data<Database>,
) -> impl Responder {
    println!("API Info: /api/admin/dao_auction/end - 收到提前结束 DAO 拍卖请求。");

    match db.end_dao_auction() {
        Ok(_) => {
            println!("API Success: /api/admin/dao_auction/end - DAO 拍卖已提前结束。");
            HttpResponse::Ok().json(serde_json::json!({"message": "DAO 拍卖已提前结束"}))
        },
        Err(e) => {
            eprintln!("API Error: /api/admin/dao_auction/end - 提前结束 DAO 拍卖失败: {:?}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({"error": "提前结束 DAO 拍卖失败"}))
        },
    }
}

// 获取所有 DAO 拍卖历史
#[get("/dao_auctions/history")]
pub async fn get_all_dao_auctions_admin(
    db: web::Data<Database>,
) -> impl Responder {
    println!("API Info: /api/admin/dao_auctions/history - 收到获取所有 DAO 拍卖历史请求。");
    match db.get_all_dao_auctions() {
        Ok(auctions) => {
            println!("API Success: /api/admin/dao_auctions/history - 已获取 {} 条 DAO 拍卖历史记录。", auctions.len());
            HttpResponse::Ok().json(auctions)
        },
        Err(e) => {
            eprintln!("API Error: /api/admin/dao_auctions/history - 获取 DAO 拍卖历史失败: {:?}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({"error": "获取 DAO 拍卖历史失败"}))
        },
    }
}

// 获取所有绑定的 BSC 地址对应用户列表 
#[get("/user_bsc_addresses")]
pub async fn get_all_user_bsc_addresses(
    db: web::Data<Database>,
) -> impl Responder {
    println!("API Info: /api/admin/user_bsc_addresses - 收到获取所有用户 BSC 地址列表请求。");

    match db.get_all_user_bsc_addresses() {
        Ok(addresses) => {
            println!("API Success: /api/admin/user_bsc_addresses - 已获取 {} 条用户 BSC 地址记录。", addresses.len());
            HttpResponse::Ok().json(addresses)
        },
        Err(e) => {
            eprintln!("API Error: /api/admin/user_bsc_addresses - 获取用户 BSC 地址列表失败: {:?}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({"error": "获取用户 BSC 地址列表失败"}))
        },
    }
}

// 管理员发布学院文章
#[post("/academy/articles")]
pub async fn publish_article(
    db: web::Data<Database>,
    req: web::Json<CreateArticleRequest>,
) -> impl Responder {
    println!("API Info: /api/admin/academy/articles - 收到发布新文章请求。");
    if req.title.is_empty() || req.summary.is_empty() || req.content.is_empty() {
        eprintln!("API Error: /api/admin/academy/articles - 标题、摘要或内容为空。");
        return HttpResponse::BadRequest().json(serde_json::json!({"error": "文章标题、摘要和内容不能为空"}));
    }

    match db.create_academy_article(&req.title, &req.summary, req.image_url.as_deref(), req.is_displayed, &req.content) {
        Ok(article_id) => {
            println!("API Success: /api/admin/academy/articles - 文章发布成功，ID: {}", article_id);
            HttpResponse::Created().json(serde_json::json!({"message": "文章发布成功", "id": article_id}))
        },
        Err(e) => {
            eprintln!("API Error: /api/admin/academy/articles - 发布文章失败: {:?}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({"error": "发布文章失败"}))
        },
    }
}

// 管理员修改学院文章 (已存在)
#[post("/academy/articles/{id}")]
pub async fn modify_article(
    db: web::Data<Database>,
    path: web::Path<i64>,
    req: web::Json<UpdateArticleRequest>,
) -> impl Responder {
    let article_id = path.into_inner();
    println!("API Info: /api/admin/academy/articles/{} - 收到修改文章请求。", article_id);

    // 验证文章标题、摘要、内容不为空
    if req.title.is_empty() || req.summary.is_empty() || req.content.is_empty() {
        eprintln!("API Error: /api/admin/academy/articles/{} - 标题、摘要或内容为空。", article_id);
        return HttpResponse::BadRequest().json(serde_json::json!({"error": "文章标题、摘要和内容不能为空"}));
    }

    match db.update_academy_article(article_id, &req.title, &req.summary, req.image_url.as_deref(), req.is_displayed, &req.content) {
        Ok(_) => {
            println!("API Success: /api/admin/academy/articles/{} - 文章修改成功。", article_id);
            HttpResponse::Ok().json(serde_json::json!({"message": "文章修改成功"}))
        },
        Err(e) => {
            eprintln!("API Error: /api/admin/academy/articles/{} - 修改文章失败: {:?}", article_id, e);
            HttpResponse::InternalServerError().json(serde_json::json!({"error": "修改文章失败"}))
        },
    }
}

// 管理员删除学院文章
#[delete("/academy/articles/{id}")]
pub async fn delete_article(
    db: web::Data<Database>,
    path: web::Path<i64>,
) -> impl Responder {
    let article_id = path.into_inner();
    println!("API Info: /api/admin/academy/articles/{} - 收到删除文章请求。", article_id);

    match db.delete_academy_article(article_id) {
        Ok(_) => {
            println!("API Success: /api/admin/academy/articles/{} - 文章删除成功。", article_id);
            HttpResponse::Ok().json(serde_json::json!({"message": "文章删除成功"}))
        },
        Err(e) => {
            eprintln!("API Error: /api/admin/academy/articles/{} - 删除文章失败: {:?}", article_id, e);
            HttpResponse::InternalServerError().json(serde_json::json!({"error": "删除文章失败"}))
        },
    }
}

//管理员获取所有学院文章列表 (包括未显示的)
#[get("/academy/articles/all")]
pub async fn get_all_articles_admin(
    db: web::Data<Database>,
) -> impl Responder {
    println!("API Info: /api/admin/academy/articles/all - 收到获取所有文章列表请求 (管理员)。");

    match db.get_all_academy_articles_admin() { // 调用 db 中获取所有文章的函数
        Ok(articles) => {
            println!("API Success: /api/admin/academy/articles/all - 已获取 {} 篇文章摘要 (管理员)。", articles.len());
            HttpResponse::Ok().json(articles)
        },
        Err(e) => {
            eprintln!("API Error: /api/admin/academy/articles/all - 获取所有文章列表失败 (管理员): {:?}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({"error": "获取所有文章列表失败"}))
        },
    }
}

// 管理员根据 ID 获取学院文章详情 (包含 content)
#[get("/academy/articles/{id}")]
pub async fn get_article_detail_admin(
    db: web::Data<Database>,
    path: web::Path<i64>,
) -> impl Responder {
    let article_id = path.into_inner();
    println!("API Call: /api/admin/academy/articles/{} - 收到获取文章详情请求 (管理员)。", article_id);

    match db.get_academy_article_by_id(article_id) {
        Ok(Some(article)) => {
            println!("API Success: /api/admin/academy/articles/{} - 已获取文章详情 (管理员)。", article_id);
            HttpResponse::Ok().json(article)
        },
        Ok(None) => {
            eprintln!("API Error: /api/admin/academy/articles/{} - 未找到文章 (管理员)。", article_id);
            HttpResponse::NotFound().json(serde_json::json!({"error": "文章未找到"}))
        },
        Err(e) => {
            eprintln!("API Error: /api/admin/academy/articles/{} - 获取文章详情失败 (管理员): {:?}", article_id, e);
            HttpResponse::InternalServerError().json(serde_json::json!({"error": "获取文章详情失败"}))
        },
    }
}

// 获取所有推荐关系
#[get("/referrals/all")]
pub async fn get_all_referral_relationships_admin(
    db: web::Data<Database>,
) -> impl Responder {
    println!("API Info: /api/admin/referrals/all - 收到获取所有推荐关系请求。");
    match db.get_all_referral_relationships() {
        Ok(relationships) => {
            println!("API Success: /api/admin/referrals/all - 已获取 {} 条推荐关系。", relationships.len());
            HttpResponse::Ok().json(relationships)
        },
        Err(e) => {
            eprintln!("API Error: /api/admin/referrals/all - 获取所有推荐关系失败: {:?}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({"error": "获取所有推荐关系失败"}))
        },
    }
}

// 获取所有佣金记录
#[get("/commissions/all")]
pub async fn get_all_commissions_admin(
    db: web::Data<Database>,
) -> impl Responder {
    println!("API Info: /api/admin/commissions/all - 收到获取所有佣金记录请求。");
    match db.get_all_commission_records_admin() {
        Ok(records) => {
            println!("API Success: /api/admin/commissions/all - 已获取 {} 条佣金记录。", records.len());
            HttpResponse::Ok().json(records)
        },
        Err(e) => {
            eprintln!("API Error: /api/admin/commissions/all - 获取所有佣金记录失败: {:?}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({"error": "获取所有佣金记录失败"}))
        },
    }
}

// 按邀请人汇总佣金数据
#[get("/commissions/summary_by_inviter")]
pub async fn get_commissions_summary_by_inviter_admin(
    db: web::Data<Database>,
) -> impl Responder {
    println!("API Info: /api/admin/commissions/summary_by_inviter - 收到按邀请人汇总佣金数据请求。");
    match db.get_commission_summary_by_inviter() {
        Ok(summary) => {
            println!("API Success: /api/admin/commissions/summary_by_inviter - 已获取 {} 条按邀请人汇总的佣金数据。", summary.len());
            HttpResponse::Ok().json(summary)
        },
        Err(e) => {
            eprintln!("API Error: /api/admin/commissions/summary_by_inviter - 获取按邀请人汇总的佣金数据失败: {:?}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({"error": "获取按邀请人汇总的佣金数据失败"}))
        },
    }
}

// 获取财务汇总信息
#[get("/financial_summary")]
pub async fn get_financial_summary_admin(
    db: web::Data<Database>,
) -> impl Responder {
    println!("API Info: /api/admin/financial_summary - 收到获取财务汇总信息请求。");
    match db.get_financial_summary() {
        Ok(summary) => {
            println!("API Success: /api/admin/financial_summary - 已获取财务汇总信息。");
            HttpResponse::Ok().json(summary)
        },
        Err(e) => {
            eprintln!("API Error: /api/admin/financial_summary - 获取财务汇总信息失败: {:?}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({"error": "获取财务汇总信息失败"}))
        },
    }
}

//获取所有用户（邮箱）、BSC 地址和 GNTX 数量
#[get("/users/gntx_info")]
pub async fn get_all_user_gntx_info(
    db: web::Data<Database>,
) -> impl Responder {
    println!("API Info: /api/admin/users/gntx_info - 收到获取所有用户 GNTX 信息请求。");
    match db.get_all_user_bsc_addresses_with_gntx() {
        Ok(info) => {
            println!("API Success: /api/admin/users/gntx_info - 已获取 {} 条用户 GNTX 信息。", info.len());
            HttpResponse::Ok().json(info)
        },
        Err(e) => {
            eprintln!("API Error: /api/admin/users/gntx_info - 获取所有用户 GNTX 信息失败: {:?}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({"error": "获取用户 GNTX 信息失败"}))
        },
    }
}

//更新用户的 GNTX 数量
#[put("/users/gntx_balance")]
pub async fn update_user_gntx_balance_admin(
    db: web::Data<Database>,
    req: web::Json<UpdateGntxBalanceRequest>,
) -> impl Responder {
    println!("API Info: /api/admin/users/gntx_balance - 收到更新用户 GNTX 数量请求。");

    let email = &req.email;
    let gntx_balance = req.gntx_balance;

    if !crate::utils::is_valid_email(email) {
        eprintln!("API Error: /api/admin/users/gntx_balance - 提供无效的邮箱格式: {}", email);
        return HttpResponse::BadRequest().json(serde_json::json!({"error": "邮箱格式不正确"}));
    }

    if gntx_balance < 0.0 {
        eprintln!("API Error: /api/admin/users/gntx_balance - GNTX 数量不能为负数: {}", gntx_balance);
        return HttpResponse::BadRequest().json(serde_json::json!({"error": "GNTX 数量不能为负数"}));
    }

    match db.update_user_gntx_balance_by_email(email, gntx_balance) {
        Ok(_) => {
            println!("API Success: /api/admin/users/gntx_balance - 已成功更新用户 {} 的 GNTX 数量为 {}。", email, gntx_balance);
            HttpResponse::Ok().json(serde_json::json!({"message": format!("GNTX 数量已成功更新为 {}", gntx_balance)}))
        },
        Err(e) => {
            eprintln!("API Error: /api/admin/users/gntx_balance - 更新用户 {} 的 GNTX 数量失败: {:?}", email, e);
            HttpResponse::InternalServerError().json(serde_json::json!({"error": "更新 GNTX 数量失败"}))
        },
    }
}

// 获取指定交易所下所有用户绑定的 UID 列表
#[get("/exchanges/{exchange_id}/users")]
pub async fn get_exchange_bound_users_admin(
    db: web::Data<Database>,
    path: web::Path<i64>,
) -> impl Responder {
    let exchange_id = path.into_inner();
    println!("API Info: /api/admin/exchanges/{}/users - 收到获取指定交易所绑定用户UID请求。", exchange_id);

    match db.get_exchange_bound_users(exchange_id) {
        Ok(users) => {
            println!("API Success: /api/admin/exchanges/{}/users - 成功获取 {} 条绑定用户UID信息。", exchange_id, users.len());
            HttpResponse::Ok().json(users)
        },
        Err(e) => {
            eprintln!("API Error: /api/admin/exchanges/{}/users - 获取指定交易所绑定用户UID失败: {:?}", exchange_id, e);
            HttpResponse::InternalServerError().json(serde_json::json!({"error": "获取绑定用户UID失败"}))
        },
    }
}

// 新增：更新NTX分配控制的目标百分比
#[post("/ntx_control/update_percentage")]
pub async fn update_ntx_control_percentage(
    db: web::Data<Database>,
    req: web::Json<UpdateNtxControlRequest>,
) -> impl Responder {
    println!("API Info: /api/admin/ntx_control/update_percentage - 收到更新NTX控制百分比请求。");
    let percentage = req.admin_fee_percentage;

    // 数据验证：百分比应在 0 到 100 之间 (但不包括100，因为会导致除零)
    if !(0.0..100.0).contains(&percentage) {
        eprintln!("API Error: /api/admin/ntx_control/update_percentage - 无效的百分比: {}", percentage);
        return HttpResponse::BadRequest().json(serde_json::json!({"error": "百分比必须在 0.0 到 100.0 之间 (不含100.0)"}));
    }

    match db.update_ntx_control_percentage(percentage) {
        Ok(_) => {
            println!("API Success: /api/admin/ntx_control/update_percentage - NTX控制百分比已更新为 {}%。", percentage);
            HttpResponse::Ok().json(serde_json::json!({"message": "NTX 控制百分比更新成功"}))
        }
        Err(e) => {
            eprintln!("API Error: /api/admin/ntx_control/update_percentage - 更新NTX控制百分比失败: {:?}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({"error": "数据库更新失败"}))
        }
    }
}