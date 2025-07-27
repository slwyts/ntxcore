// src/mining.rs
use actix_web::{get, web, HttpResponse, Responder};
use serde::Serialize;
use crate::{db::Database, utils::is_valid_date};
use crate::JwtConfig; // 尽管不再用于此API，但可能被其他API使用，保留导入
use crate::user; // 尽管不再用于此API，但可能被其他API使用，保留导入
use actix_web::HttpRequest; // 尽管不再用于此API，但可能被其他API使用，保留导入
use actix_web::post;
use serde::Deserialize;

#[derive(Deserialize)]
pub struct DailyUserDataRequest {
    pub date: String,
}
#[derive(Deserialize)]
pub struct DailyPlatformDataRequest {
    pub date: String,
}
#[derive(Serialize)]
struct PlatformDataResponse {
    total_mined: f64,
    total_commission: f64,
    total_burned: f64,
    total_trading_volume: f64,
    platform_users: i64,
}

#[derive(Serialize)]
struct DailyPlatformDataResponse {
    mining_output: f64,
    burned: f64,
    commission: f64,
    trading_volume: f64,
    miners: i64,
}

#[derive(Deserialize)]
pub struct BindExchangeRequest {
    pub exchange_id: i64,
    pub exchange_uid: Option<String>,
}

// 获取所有交易所信息
#[get("/get_exchanges")]
pub async fn get_exchanges(
    db: web::Data<Database>,
) -> impl Responder {
    println!("API Call: /api/mining/get_exchanges 收到请求。");
    match db.get_exchanges() {
        Ok(exchanges) => {
            println!("API Success: /api/mining/get_exchanges - 已获取 {} 个交易所。", exchanges.len());
            HttpResponse::Ok().json(exchanges)
        },
        Err(e) => {
            eprintln!("API Error: /api/mining/get_exchanges - 获取交易所列表失败: {:?}", e);
            HttpResponse::InternalServerError().finish()
        }
    }
}

// 绑定用户与交易所
#[post("/bind_exchange")]
pub async fn bind_exchange(
    db: web::Data<Database>,
    jwt_config: web::Data<JwtConfig>,
    req: HttpRequest,
    bind_req: web::Json<BindExchangeRequest>,
) -> impl Responder {
    // 1. 验证用户身份
    let user_id = match user::get_user_id_from_token(&req, &jwt_config) {
        Ok(id) => id,
        Err(resp) => {
            eprintln!("API Error: /api/mining/bind_exchange - 未授权访问。");
            return resp;
        },
    };
    println!("API Info: /api/mining/bind_exchange - 用户ID {} 请求操作交易所 {}。", user_id, bind_req.exchange_id);

    // 2. 根据 exchange_uid 的内容决定是绑定还是解绑
    match &bind_req.exchange_uid {
        // 如果 UID 存在且不为空，则为绑定/更新操作
        Some(uid) if !uid.is_empty() => {
            println!("API Info: /api/mining/bind_exchange - 用户 {} 正在绑定/更新交易所 {}，UID: {}", user_id, bind_req.exchange_id, uid);
            match db.bind_user_exchange(user_id, bind_req.exchange_id, uid) {
                Ok(_) => {
                    println!("API Success: /api/mining/bind_exchange - 用户 {} 成功绑定/更新交易所 {}", user_id, bind_req.exchange_id);
                    HttpResponse::Ok().json(serde_json::json!({
                        "message": "交易所绑定或更新成功"
                    }))
                },
                Err(e) => {
                    eprintln!("API Error: /api/mining/bind_exchange - 用户 {} 绑定/更新交易所失败: {:?}", user_id, e);
                    HttpResponse::InternalServerError().json(serde_json::json!({"error": "操作失败"}))
                }
            }
        },
        // 如果 UID 为 None 或为空字符串，则为解绑操作
        _ => {
            println!("API Info: /api/mining/bind_exchange - 用户 {} 正在解绑交易所 {}", user_id, bind_req.exchange_id);
            match db.unbind_user_exchange(user_id, bind_req.exchange_id) {
                Ok(_) => {
                    println!("API Success: /api/mining/bind_exchange - 用户 {} 成功解绑交易所 {}", user_id, bind_req.exchange_id);
                    HttpResponse::Ok().json(serde_json::json!({
                        "message": "交易所解绑成功"
                    }))
                },
                Err(e) => {
                    eprintln!("API Error: /api/mining/bind_exchange - 用户 {} 解绑交易所失败: {:?}", user_id, e);
                    HttpResponse::InternalServerError().json(serde_json::json!({"error": "操作失败"}))
                }
            }
        }
    }
}
// 获取平台总数据
#[get("/platform_data")]
pub async fn get_platform_data(
    db: web::Data<Database>,
) -> impl Responder {
    println!("API Call: /api/mining/platform_data 收到请求。");
    match db.get_platform_data() { // 调用 db.rs 中的 get_platform_data 方法
        Ok(data) => {
            println!("API Success: /api/mining/platform_data - 已获取平台数据。");
            HttpResponse::Ok().json(PlatformDataResponse {
                total_mined: data.total_mined,
                total_commission: data.total_commission,
                total_burned: data.total_burned,
                total_trading_volume: data.total_trading_volume,
                platform_users: data.platform_users,
            })
        },
        Err(e) => {
            eprintln!("API Error: /api/mining/platform_data - 获取平台数据失败: {:?}", e); // 记录错误以便调试
            // 如果获取失败，返回默认值
            HttpResponse::Ok().json(PlatformDataResponse {
                total_mined: 0.0,
                total_commission: 0.0,
                total_burned: 0.0,
                total_trading_volume: 0.0,
                platform_users: 0,
            })
        }
    }
}

// 获取每日平台数据
#[get("/daily_platform_data")]
pub async fn get_daily_platform_data(
    db: web::Data<Database>,
    // 修改这里，使用 DailyPlatformDataRequest 结构体来接收查询参数
    query: web::Query<DailyPlatformDataRequest>,
) -> impl Responder {
    let date_str = query.date.clone(); // 从结构体中获取 date 字段
    println!("API Call: /api/mining/daily_platform_data 收到请求，日期: {}", date_str);

    if !is_valid_date(&date_str) {
        eprintln!("API Error: /api/mining/daily_platform_data - 无效的日期格式: {}", date_str);
        return HttpResponse::BadRequest().json(
            serde_json::json!({"error": "无效的日期格式，应为YYYY-MM-DD"})
        );
    }

    match db.get_daily_platform_data(&date_str) {
        Ok(Some(data)) => {
            println!("API Success: /api/mining/daily_platform_data - 已获取 {} 的每日平台数据。", date_str);
            HttpResponse::Ok().json(DailyPlatformDataResponse {
                mining_output: data.mining_output,
                burned: data.burned,
                commission: data.commission,
                trading_volume: data.trading_volume,
                miners: data.miners,
            })
        },
        Ok(None) => {
            println!("API Info: /api/mining/daily_platform_data - 未找到日期 {} 的平台数据。返回默认值。", date_str);
            // 未找到数据时，返回默认值
            HttpResponse::Ok().json(DailyPlatformDataResponse {
                mining_output: 0.0,
                burned: 0.0,
                commission: 0.0,
                trading_volume: 0.0,
                miners: 0,
            })
        },
        Err(e) => {
            eprintln!("API Error: /api/mining/daily_platform_data - 获取日期 {} 的每日平台数据失败: {:?}", date_str, e);
            // 数据库查询出错时，也返回默认值
            HttpResponse::Ok().json(DailyPlatformDataResponse {
                mining_output: 0.0,
                burned: 0.0,
                commission: 0.0,
                trading_volume: 0.0,
                miners: 0,
            })
        }
    }
}

// 获取用户绑定的交易所
#[get("/user_exchanges")]
pub async fn get_user_exchanges(
    db: web::Data<Database>,
    jwt_config: web::Data<JwtConfig>,
    req: HttpRequest,
) -> impl Responder {
    println!("API Call: /api/mining/user_exchanges 收到请求。");

    // 验证用户身份
    let user_id = match user::get_user_id_from_token(&req, &jwt_config) {
        Ok(id) => id,
        Err(resp) => {
            eprintln!("API Error: /api/mining/user_exchanges - 未授权访问。");
            return resp;
        },
    };
    println!("API Info: /api/mining/user_exchanges - 用户ID {} 请求绑定的交易所。", user_id);

    // 获取用户绑定的交易所
    match db.get_user_exchanges(user_id) {
        Ok(exchanges) => {
            println!("API Success: /api/mining/user_exchanges - 用户 {} 已获取 {} 个绑定的交易所。", user_id, exchanges.len());
            HttpResponse::Ok().json(exchanges)
        },
        Err(e) => {
            eprintln!("API Error: /api/mining/user_exchanges - 获取用户 {} 绑定的交易所失败: {:?}", user_id, e);
            HttpResponse::InternalServerError().finish()
        }
    }
}

// 获取用户总数据
#[get("/user_data")]
pub async fn get_user_data(
    db: web::Data<Database>,
    jwt_config: web::Data<JwtConfig>,
    req: HttpRequest,
) -> impl Responder {
    println!("API Call: /api/mining/user_data 收到请求。");

    let user_id = match user::get_user_id_from_token(&req, &jwt_config) {
        Ok(id) => id,
        Err(resp) => {
            eprintln!("API Error: /api/mining/user_data - 未授权访问。");
            return resp;
        },
    };
    println!("API Info: /api/mining/user_data - 用户ID {} 请求总数据。", user_id);

    match db.get_user_data(user_id) {
        Ok(Some(data)) => {
            println!("API Success: /api/mining/user_data - 用户 {} 已获取总数据。", user_id);
            HttpResponse::Ok().json(data)
        },
        Ok(None) => {
            println!("API Info: /api/mining/user_data - 未找到用户 {} 的总数据。返回默认值。", user_id);
            // 未找到数据时，返回默认值
            HttpResponse::Ok().json(crate::db::UserData {
                total_mining: 0.0,
                total_trading_cost: 0.0,
            })
        },
        Err(e) => {
            eprintln!("API Error: /api/mining/user_data - 获取用户 {} 总数据失败: {:?}", user_id, e);
            // 数据库查询出错时，也返回默认值
            HttpResponse::Ok().json(crate::db::UserData {
                total_mining: 0.0,
                total_trading_cost: 0.0,
            })
        }
    }
}

// 获取每日用户数据
#[get("/daily_user_data")]
pub async fn get_daily_user_data(
    db: web::Data<Database>,
    jwt_config: web::Data<JwtConfig>,
    req: HttpRequest,
    query: web::Query<DailyUserDataRequest>,
) -> impl Responder {
    let date_str = query.date.clone();
    println!("API Call: /api/mining/daily_user_data 收到请求，日期: {}", date_str);

    let user_id = match user::get_user_id_from_token(&req, &jwt_config) {
        Ok(id) => id,
        Err(resp) => {
            eprintln!("API Error: /api/mining/daily_user_data - 未授权访问。");
            return resp;
        },
    };
    println!("API Info: /api/mining/daily_user_data - 用户ID {} 请求日期 {} 的每日数据。", user_id, date_str);

    if !is_valid_date(&date_str) {
        eprintln!("API Error: /api/mining/daily_user_data - 用户 {} 的日期格式无效: {}", user_id, date_str);
        return HttpResponse::BadRequest().json(
            serde_json::json!({"error": "无效的日期格式，应为YYYY-MM-DD"})
        );
    }

    match db.get_daily_user_data(user_id, &date_str) {
        Ok(Some(data)) => {
            println!("API Success: /api/mining/daily_user_data - 用户 {} 已获取日期 {} 的每日数据。", user_id, date_str);
            HttpResponse::Ok().json(data)
        },
        Ok(None) => {
            println!("API Info: /api/mining/daily_user_data - 未找到用户 {} 在日期 {} 的每日数据。返回默认值。", user_id, date_str);
            // 未找到数据时，返回默认值
            HttpResponse::Ok().json(crate::db::DailyUserData {
                mining_output: 0.0,
                total_trading_cost: 0.0,
            })
        },
        Err(e) => {
            eprintln!("API Error: /api/mining/daily_user_data - 获取用户 {} 在日期 {} 的每日数据失败: {:?}", user_id, date_str, e);
            // 数据库查询出错时，也返回默认值
            HttpResponse::Ok().json(crate::db::DailyUserData {
                mining_output: 0.0,
                total_trading_cost: 0.0,
            })
        }
    }
}

// 获取全平台挖矿NTX总数量前十名信息
#[get("/mining_leaderboard")]
pub async fn get_mining_leaderboard(
    db: web::Data<Database>,
    // 移除 JWT 相关的参数和验证，使其成为公共API
    // jwt_config: web::Data<JwtConfig>,
    // req: HttpRequest,
) -> impl Responder {
    println!("API Call: /api/mining/mining_leaderboard received.");

    // 之前这里有验证用户身份的代码，现在已移除，使其成为公共API。
    // let _user_id = match user::get_user_id_from_token(&req, &jwt_config) {
    //     Ok(id) => id,
    //     Err(resp) => {
    //         eprintln!("API Error: /api/mining/mining_leaderboard - 未授权访问。");
    //         return resp;
    //     },
    // };
    println!("API Info: /api/mining/mining_leaderboard - 请求挖矿排行榜前10名。");

    match db.get_mining_leaderboard_top10() {
        Ok(leaderboard) => {
            println!("API Success: /api/mining/mining_leaderboard - 已获取挖矿排行榜数据。");
            HttpResponse::Ok().json(leaderboard)
        },
        Err(e) => {
            eprintln!("API Error: /api/mining/mining_leaderboard - 获取挖矿排行榜失败: {:?}", e);
            HttpResponse::InternalServerError().finish()
        }
    }
}
